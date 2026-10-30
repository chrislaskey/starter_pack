defmodule App.Data.SyncTest do
  use App.DataCase

  import App.Data.CommunitiesFixtures
  import App.Data.PostsFixtures
  import App.Data.UsersFixtures

  alias App.Data.Memberships
  alias App.Data.PostEvents
  alias App.Data.Posting
  alias App.Data.Posts
  alias App.Data.Sync
  alias App.Data.Synced
  alias App.SyncClient

  setup do
    author = user_scope_fixture(user_fixture())
    other = user_scope_fixture(user_fixture())
    community = joined_community_fixture(author)
    {:ok, _membership} = Memberships.join(other, community.id)
    %{author: author, other: other, community: community}
  end

  # A post written on a device, in the community: the post's create event
  # and its posts_communities row, queued together.
  defp write_post(client, body, community_id, opts \\ []) do
    {client, post_id} = SyncClient.create(client, "app_posts", %{"body" => body}, opts)

    # The membership as the device knows it, which may be stale
    user_id = client.scope.user.id

    membership_id =
      case Enum.find(
             SyncClient.rows(client, "app_users_communities"),
             &(&1["user_id"] == user_id and &1["community_id"] == community_id)
           ) do
        %{"id" => id} -> id
        nil -> Memberships.membership(client.scope, community_id).id
      end

    client =
      SyncClient.write(
        client,
        "app_posts_communities",
        %{"post_id" => post_id, "community_id" => community_id, "users_communities_id" => membership_id},
        opts
      )

    {client, post_id}
  end

  # An edit made on the server, as the web app or another device would,
  # from the version the server has
  defp edit_on_server(scope, post, changes) do
    Posting.apply_event(scope, %{
      id: Ecto.UUID.generate(version: 7),
      row_id: post.id,
      type: :update,
      base_version: Posts.get(post.id, include_deleted: true).version,
      base: %{},
      changes: changes
    })
  end

  test "every synced schema is marked and listed, parents first" do
    for schema <- Sync.schemas(), do: assert(Synced.synced?(schema))

    assert Enum.map(Sync.tables(), &elem(&1, 0)) ==
             ~w(app_users app_communities app_users_communities app_posts app_posts_communities)
  end

  describe "pull/2" do
    test "delivers what the user can see, across tables, in cursor order with a page token",
         %{author: author, other: other, community: community} do
      post_fixture(other, [community.id])
      post_fixture(author, [community.id])
      post_fixture(author, [])

      phone = SyncClient.pull(SyncClient.new(author), limit: 3)

      assert length(phone.last_pull) > 1

      assert Enum.map(SyncClient.rows(phone, "app_users"), & &1["id"]) ==
               Enum.sort([author.user.id, other.user.id])

      assert [%{"id" => community_id}] = SyncClient.rows(phone, "app_communities")
      assert community_id == community.id
      assert length(SyncClient.rows(phone, "app_users_communities")) == 2
      assert length(SyncClient.rows(phone, "app_posts")) == 3
      assert length(SyncClient.rows(phone, "app_posts_communities")) == 2
      assert %{"last_synced_at" => <<_::binary>>, "id" => <<_::binary>>} = phone.cursor

      # Every page ends later than the one before, and the last one has no
      # next page
      cursors = for page <- phone.last_pull, do: {page["cursor"]["last_synced_at"], page["cursor"]["id"]}
      assert cursors == Enum.sort(cursors) and cursors == Enum.uniq(cursors)
      assert List.last(phone.last_pull)["next"] == nil
    end

    test "the overlap buffer re-delivers without duplicating", %{author: author, community: community} do
      post = post_fixture(author, [community.id])
      phone = SyncClient.pull(SyncClient.new(author))
      assert SyncClient.row(phone, "app_posts", post.id)

      again = SyncClient.pull(phone)

      assert [page] = again.last_pull
      assert post.id in for(%{"row" => row} <- page["rows"], do: row["id"])
      assert again.rows == phone.rows
      assert again.cursor == phone.cursor
    end

    test "a posts_communities row for a newly joined community arrives with its post and author",
         %{author: author, other: other, community: community} do
      phone = SyncClient.pull(SyncClient.new(author))
      old = post_fixture(other, [])
      phone = SyncClient.pull(phone)
      refute SyncClient.row(phone, "app_posts", old.id)

      {:ok, _row} = Posting.add_to_community(other, old, community.id)
      phone = SyncClient.pull(phone)

      assert SyncClient.row(phone, "app_posts", old.id)["body"] == old.body
      assert SyncClient.row(phone, "app_users", other.user.id)

      # The author and the post come before the row that carries them, once
      [page] = phone.last_pull
      keys = for %{"table" => table, "row" => row} <- page["rows"], do: {table, row["id"]}
      [row] = for {"app_posts_communities", id} <- keys, do: id
      assert keys == Enum.uniq(keys)

      assert Enum.find_index(keys, &(&1 == {"app_users", other.user.id})) <
               Enum.find_index(keys, &(&1 == {"app_posts", old.id}))

      assert Enum.find_index(keys, &(&1 == {"app_posts", old.id})) <
               Enum.find_index(keys, &(&1 == {"app_posts_communities", row}))
    end

    test "a post removed from a community and a left membership arrive as deletes",
         %{author: author, other: other, community: community} do
      post = post_fixture(other, [community.id])
      phone = SyncClient.pull(SyncClient.new(author))

      {:ok, _row} = Posting.remove_from_community(other, post, community.id)
      phone = SyncClient.pull(phone)
      [row] = for row <- SyncClient.rows(phone, "app_posts_communities"), row["post_id"] == post.id, do: row
      assert row["deleted_at"]

      {:ok, membership} = Memberships.leave(author, community.id)
      phone = SyncClient.pull(phone)
      assert SyncClient.row(phone, "app_users_communities", membership.id)["deleted_at"]
    end
  end

  describe "push/2" do
    test "a post is created by an event, and an edit from the version the server has applies",
         %{author: author, community: community} do
      {phone, post_id} = write_post(SyncClient.new(author), "draft", community.id)

      {phone,
       %{
         "events" => [%{"row_id" => ^post_id, "status" => "applied", "version" => 1}],
         "rejected" => [%{"table" => "app_posts_communities", "reason" => "server_wins"}]
       }} = SyncClient.push(phone)

      assert SyncClient.row(phone, "app_posts", post_id)["version"] == 1

      phone = SyncClient.change(phone, "app_posts", post_id, %{"body" => "edited"})
      {phone, %{"events" => [%{"status" => "applied", "version" => 2}]}} = SyncClient.push(phone)

      assert Posts.get(post_id).body == "edited"
      assert SyncClient.row(phone, "app_posts", post_id)["version"] == 2

      assert [%{type: :create, base_version: 0, version: 1}, %{type: :update, base_version: 1, version: 2}] =
               PostEvents.list(filters: [post_id: post_id])
    end

    test "the same field edited on two devices keeps the edit that reached the server first, and the other becomes a conflict",
         %{author: author, community: community} do
      post = post_fixture(author, [community.id], %{body: "draft"})
      phone = SyncClient.pull(SyncClient.new(author))
      laptop = SyncClient.pull(SyncClient.new(author))

      # The phone is offline all along; the laptop edits and syncs first
      phone = SyncClient.change(phone, "app_posts", post.id, %{"body" => "from the phone"})
      laptop = SyncClient.change(laptop, "app_posts", post.id, %{"body" => "from the laptop"})
      {_laptop, %{"events" => [%{"status" => "applied", "version" => 2}]}} = SyncClient.push(laptop)

      {phone, %{"events" => [answer]}} = SyncClient.push(phone)

      assert %{
               "status" => "conflict",
               "version" => 2,
               "conflicts" => %{"body" => %{"yours" => "from the phone", "server" => "from the laptop"}}
             } = answer

      assert Posts.get(post.id).body == "from the laptop"
      assert SyncClient.row(phone, "app_posts", post.id)["body"] == "from the laptop"
      assert [%{"conflicts" => %{"body" => _}}] = phone.conflicts

      # The rejected change is in the post's history with nothing applied
      assert %{changes: changes, conflicts: %{"body" => _}, version: 2} =
               PostEvents.get(answer["id"])

      assert changes == %{}
    end

    test "a conflict reapplied from the server's version wins, and one ignored changes nothing",
         %{author: author, community: community} do
      post = post_fixture(author, [community.id], %{body: "draft"})
      phone = SyncClient.pull(SyncClient.new(author))

      {:ok, _event, _post} = edit_on_server(author, post, %{"body" => "on the server"})
      phone = SyncClient.change(phone, "app_posts", post.id, %{"body" => "mine"})
      {phone, _response} = SyncClient.push(phone)
      [conflict] = phone.conflicts

      ignored = SyncClient.resolve(phone, conflict["id"], :ignore)
      assert ignored.conflicts == [] and ignored.events == []

      phone = SyncClient.resolve(phone, conflict["id"], :reapply)
      assert [%{"base_version" => 2, "base" => %{"body" => "on the server"}}] = phone.events
      {phone, %{"events" => [%{"status" => "applied", "version" => 3}]}} = SyncClient.push(phone)

      assert Posts.get(post.id).body == "mine"
      assert phone.conflicts == []
    end

    test "different fields edited on two devices both apply", %{author: author, community: community} do
      post = post_fixture(author, [community.id], %{body: "draft"})
      phone = SyncClient.pull(SyncClient.new(author))

      {:ok, _event, _post} = edit_on_server(author, post, %{"body" => "edited on the laptop"})

      deleted_at = DateTime.to_iso8601(DateTime.utc_now())
      phone = SyncClient.change(phone, "app_posts", post.id, %{"deleted_at" => deleted_at})
      {_phone, %{"events" => [answer]}} = SyncClient.push(phone)

      assert %{"status" => "applied", "version" => 3, "conflicts" => %{}} = answer
      deleted = Posts.get(post.id, include_deleted: true)
      assert deleted.body == "edited on the laptop"
      assert DateTime.to_iso8601(deleted.deleted_at) == deleted_at
      assert deleted.deleted_by_id == author.user.id
    end

    test "edits made one after another offline do not conflict with each other",
         %{author: author, community: community} do
      post = post_fixture(author, [community.id], %{body: "draft"})
      phone = SyncClient.pull(SyncClient.new(author))

      phone = SyncClient.change(phone, "app_posts", post.id, %{"body" => "first"})
      {phone, %{"events" => [%{"status" => "applied"}]}} = SyncClient.push(phone)

      # Queued from version 1 again, before the push answered
      phone = %{phone | rows: put_in(phone.rows, ["app_posts", post.id, "version"], 1)}
      phone = SyncClient.change(phone, "app_posts", post.id, %{"body" => "second"})
      phone = SyncClient.change(phone, "app_posts", post.id, %{"body" => "third"})
      {phone, %{"events" => answers}} = SyncClient.push(phone)

      assert Enum.map(answers, & &1["status"]) == ["applied", "applied"]
      assert Posts.get(post.id).body == "third"
      assert phone.conflicts == []
    end

    test "a retried event gets the same answer and applies nothing twice",
         %{author: author, other: other, community: community} do
      post = post_fixture(author, [community.id], %{body: "draft"})
      phone = SyncClient.pull(SyncClient.new(author))
      phone = SyncClient.change(phone, "app_posts", post.id, %{"body" => "once"})
      [event] = phone.events

      assert %{"events" => [%{"status" => "applied", "version" => 2}]} = wire(Sync.push(author, [], [event]))
      assert %{"events" => [%{"status" => "applied", "version" => 2}]} = wire(Sync.push(author, [], [event]))
      assert Posts.get(post.id).version == 2
      assert length(PostEvents.list(filters: [post_id: post.id])) == 2

      assert %{"events" => [%{"status" => "rejected", "reason" => "event_id_taken"}]} =
               wire(Sync.push(other, [], [event]))
    end

    test "a future-dated deletion in an event is bounded to server time", %{
      author: author,
      community: community
    } do
      post = post_fixture(author, [community.id])
      phone = SyncClient.pull(SyncClient.new(author, clock_offset: 24 * 60 * 60))
      future = DateTime.utc_now() |> DateTime.add(1, :day) |> DateTime.to_iso8601()
      phone = SyncClient.change(phone, "app_posts", post.id, %{"deleted_at" => future})

      {_phone, %{"events" => [%{"status" => "applied"}]}} = SyncClient.push(phone)

      deleted = Posts.get(post.id, include_deleted: true)
      assert DateTime.compare(deleted.deleted_at, DateTime.utc_now()) != :gt
    end

    test "a row pushed for a community the user left is rejected", %{author: author, community: community} do
      phone = SyncClient.pull(SyncClient.new(author))
      {:ok, _membership} = Memberships.leave(author, community.id)

      {phone, post_id} = write_post(phone, "too late", community.id)
      {phone, response} = SyncClient.push(phone)

      assert [%{"row_id" => ^post_id, "status" => "applied"}] = response["events"]

      assert [%{"table" => "app_posts_communities", "reason" => "not_a_member", "server" => nil}] =
               response["rejected"]

      assert Posting.feed(App.Web.Scope.put_community(author, community)) == []
      assert SyncClient.rows(phone, "app_posts_communities") == []
    end

    test "memberships and community rows are server-wins", %{author: author, community: community} do
      phone = SyncClient.pull(SyncClient.new(author))

      [membership] =
        Enum.filter(SyncClient.rows(phone, "app_users_communities"), &(&1["user_id"] == author.user.id))

      phone = SyncClient.delete(phone, "app_users_communities", membership["id"])
      {phone, %{"rejected" => [%{"reason" => "server_wins", "server" => server}]}} = SyncClient.push(phone)

      assert server["row"]["id"] == membership["id"]
      assert server["row"]["deleted_at"]
      refute Memberships.member?(author, community.id)
      assert SyncClient.row(phone, "app_users_communities", membership["id"])["deleted_at"]

      phone =
        SyncClient.write(phone, "app_users_communities", %{"id" => membership["id"], "deleted_at" => nil})

      {_phone, %{"rejected" => [%{"reason" => "server_wins", "server" => server}]}} = SyncClient.push(phone)
      assert server["row"]["deleted_at"] == nil
      assert Memberships.member?(author, community.id)
    end

    test "a datetime in the wrong format is rejected, in a row and in an event", %{author: author} do
      row = %{"id" => Ecto.UUID.generate(version: 7), "name" => "x", "updated_at" => "2026-10-25T22:31:55Z"}

      assert %{"rejected" => [%{"reason" => "bad_datetime: updated_at"}]} =
               Sync.push(author, [%{"table" => "app_communities", "row" => row}])

      phone = SyncClient.new(author)
      {phone, _id} = SyncClient.create(phone, "app_posts", %{"body" => "x"})
      [event] = phone.events
      event = %{event | "type" => "update", "changes" => %{"deleted_at" => "2026-10-25T22:31:55Z"}}

      assert %{"events" => [%{"status" => "rejected", "reason" => "bad_datetime: deleted_at"}]} =
               Sync.push(author, [], [event])
    end

    test "another user's post, a post as a row, and an app user row are refused",
         %{author: author, other: other} do
      theirs = post_fixture(other, [])
      now = DateTime.to_iso8601(DateTime.utc_now())

      event = %{
        "id" => Ecto.UUID.generate(version: 7),
        "table" => "app_posts",
        "row_id" => theirs.id,
        "type" => "update",
        "base_version" => theirs.version,
        "base" => %{"body" => theirs.body},
        "changes" => %{"body" => "mine now"}
      }

      assert %{
               "events" => [
                 %{
                   "status" => "rejected",
                   "reason" => "not_author",
                   "server" => %{"row" => %{"body" => body}}
                 }
               ]
             } =
               wire(Sync.push(author, [], [event]))

      assert body == theirs.body

      row = %{"id" => theirs.id, "body" => "mine now", "author_id" => author.user.id, "updated_at" => now}

      assert %{"rejected" => [%{"reason" => "events_only"}]} =
               wire(Sync.push(author, [%{"table" => "app_posts", "row" => row}]))

      assert Posts.get(theirs.id).body == theirs.body

      assert %{"rejected" => [%{"reason" => "server_wins", "server" => %{"row" => %{"email" => email}}}]} =
               wire(
                 Sync.push(author, [
                   %{
                     "table" => "app_users",
                     "row" => %{
                       "id" => author.user.id,
                       "email" => "new@example.com",
                       "joined_at" => now,
                       "updated_at" => now
                     }
                   }
                 ])
               )

      assert email == author.user.email
    end

    # As JSON would carry it
    defp wire(term), do: term |> Jason.encode!() |> Jason.decode!()
  end

  describe "the lifecycle" do
    test "a device of another member receives an account deletion through sync",
         %{author: author, other: other, community: community} do
      post = post_fixture(author, [community.id])
      phone = SyncClient.pull(SyncClient.new(other))
      assert SyncClient.row(phone, "app_posts", post.id)

      [membership] =
        Enum.filter(SyncClient.rows(phone, "app_users_communities"), &(&1["user_id"] == author.user.id))

      {:ok, _user} = App.Data.Accounts.delete_user(author.user)
      phone = SyncClient.pull(phone)

      assert SyncClient.row(phone, "app_users_communities", membership["id"])["deleted_at"]
      [row] = for row <- SyncClient.rows(phone, "app_posts_communities"), row["post_id"] == post.id, do: row
      assert row["deleted_at"]
    end

    test "a device away longer than the retention window resyncs from nothing and ends up matching",
         %{author: author, other: other, community: community} do
      kept = post_fixture(author, [community.id])
      gone = post_fixture(author, [community.id])
      phone = SyncClient.pull(SyncClient.new(other))
      assert SyncClient.row(phone, "app_posts", gone.id)

      {:ok, _post} = Posting.delete(author, gone)
      App.Data.Purge.run(now: DateTime.add(DateTime.utc_now(), 91, :day))
      assert Posts.get(gone.id, include_deleted: true) == nil

      phone = SyncClient.pull(phone)
      fresh = SyncClient.pull(SyncClient.new(other))

      assert [%{"resync" => true} | _] = phone.last_pull
      assert phone.rows == fresh.rows
      assert phone.cursor == fresh.cursor
      assert SyncClient.row(phone, "app_posts", kept.id)
      refute SyncClient.row(phone, "app_posts", gone.id)

      # A device in step with the server is not asked to resync, once the
      # horizon is where a real purge puts it, in the past
      App.Data.Purge.run()
      assert [%{"resync" => false}] = SyncClient.pull(phone).last_pull
    end
  end

  describe "push/2 authorization" do
    test "refuses a post with someone else's author id, a membership for another user, and a community row from a non-author",
         %{author: author, other: other, community: community} do
      theirs = post_fixture(other, [])
      now = DateTime.to_iso8601(DateTime.utc_now())
      membership = Memberships.membership(other, community.id)

      rows = [
        %{
          "table" => "app_posts",
          "row" => %{
            "id" => Ecto.UUID.generate(version: 7),
            "body" => "as them",
            "author_id" => other.user.id,
            "updated_at" => now
          }
        },
        %{
          "table" => "app_users_communities",
          "row" => %{
            "id" => Ecto.UUID.generate(version: 7),
            "user_id" => other.user.id,
            "community_id" => community.id,
            "updated_at" => now
          }
        },
        %{
          "table" => "app_posts_communities",
          "row" => %{
            "id" => Ecto.UUID.generate(version: 7),
            "post_id" => theirs.id,
            "community_id" => community.id,
            "users_communities_id" => membership.id,
            "updated_at" => now
          }
        }
      ]

      assert %{"accepted" => [], "rejected" => rejected} = wire(Sync.push(author, rows))
      # Applied parents first: the membership, then the post, then its community row
      assert Enum.map(rejected, & &1["reason"]) == ["not_own_membership", "events_only", "not_author"]
      assert Posts.list(filters: [author_id: other.user.id]) == [theirs]
      assert Posting.communities_of(other, theirs) == []
    end

    test "applies a post's event before its community row whatever the order sent, and a retried row gets the server's version",
         %{author: author, community: community} do
      now = DateTime.to_iso8601(DateTime.utc_now())
      post_id = Ecto.UUID.generate(version: 7)
      membership = Memberships.membership(author, community.id)

      row = %{
        "table" => "app_posts_communities",
        "row" => %{
          "id" => Ecto.UUID.generate(version: 7),
          "post_id" => post_id,
          "community_id" => community.id,
          "users_communities_id" => membership.id,
          "updated_at" => now
        }
      }

      event = %{
        "id" => Ecto.UUID.generate(version: 7),
        "table" => "app_posts",
        "row_id" => post_id,
        "type" => "create",
        "base_version" => 0,
        "changes" => %{"body" => "child first"}
      }

      assert %{
               "events" => [%{"row_id" => ^post_id, "status" => "applied"}],
               "rejected" => [%{"table" => "app_posts_communities", "reason" => "server_wins"}]
             } = wire(Sync.push(author, [row], [event]))

      assert [_row] = Posting.communities_of(author, Posts.get(post_id))

      community_id = Ecto.UUID.generate(version: 7)

      community_row = %{
        "table" => "app_communities",
        "row" => %{"id" => community_id, "name" => "N", "updated_at" => now}
      }

      assert %{"accepted" => [%{"version" => 1}]} = wire(Sync.push(author, [community_row]))

      assert %{"rejected" => [%{"reason" => "server_wins", "server" => %{"row" => %{"version" => 1}}}]} =
               wire(Sync.push(author, [community_row]))
    end

    test "a community is created from a client but never changed",
         %{author: author, community: community} do
      now = DateTime.utc_now()
      future = now |> DateTime.add(365, :day) |> DateTime.to_iso8601()
      id = Ecto.UUID.generate(version: 7)

      assert %{"accepted" => [%{"table" => "app_communities"}]} =
               wire(
                 Sync.push(author, [
                   %{
                     "table" => "app_communities",
                     "row" => %{"id" => id, "name" => "New", "updated_at" => DateTime.to_iso8601(now)}
                   }
                 ])
               )

      assert %{"rejected" => [%{"reason" => "server_wins", "server" => %{"row" => %{"name" => name}}}]} =
               wire(
                 Sync.push(author, [
                   %{
                     "table" => "app_communities",
                     "row" => %{"id" => community.id, "name" => "Renamed", "updated_at" => future}
                   }
                 ])
               )

      assert name == community.name
    end

    test "a row that is not a map, and a cursor that is not a datetime, are refused without crashing", %{
      author: author
    } do
      assert %{"rejected" => [%{"id" => nil, "reason" => "malformed", "table" => nil}]} =
               wire(Sync.push(author, ["nope"]))

      assert Sync.pull(author, %{"cursor" => %{"last_synced_at" => "yesterday", "id" => "x"}}) ==
               {:error, :bad_cursor}

      assert Sync.pull(author, %{"after" => %{"last_synced_at" => "2026", "id" => "x"}}) ==
               {:error, :bad_cursor}
    end
  end

  describe "pull/2 and tombstones" do
    test "a deleted post travels without its body, and a removed row carries no post",
         %{author: author, other: other, community: community} do
      deleted = post_fixture(author, [community.id], %{body: "secret, deleted"})
      removed = post_fixture(author, [community.id], %{body: "secret, removed"})
      {:ok, _post} = Posting.delete(author, deleted)
      {:ok, _row} = Posting.remove_from_community(author, removed, community.id)

      phone = SyncClient.pull(SyncClient.new(other))

      assert SyncClient.row(phone, "app_posts", deleted.id)["body"] == ""
      assert SyncClient.row(phone, "app_posts", deleted.id)["deleted_at"]
      refute SyncClient.row(phone, "app_posts", removed.id)

      [row] =
        for row <- SyncClient.rows(phone, "app_posts_communities"), row["post_id"] == removed.id, do: row

      assert row["deleted_at"]
    end

    test "a row written late inside the overlap window is still delivered, and a page holds each row once",
         %{author: author, other: other, community: community} do
      phone = SyncClient.pull(SyncClient.new(other))
      post = post_fixture(author, [community.id])
      late = DateTime.add(DateTime.utc_now(), -(Sync.overlap_seconds() - 5), :second)

      App.Repo.update_all(from(p in App.Data.Post, where: p.id == ^post.id), set: [last_synced_at: late])

      phone = SyncClient.pull(phone)
      assert SyncClient.row(phone, "app_posts", post.id)

      for page <- phone.last_pull do
        keys = for %{"table" => table, "row" => row} <- page["rows"], do: {table, row["id"]}
        assert keys == Enum.uniq(keys)
      end
    end
  end

  describe "backfill/3" do
    test "returns a community's existing posts, paged", %{author: author, other: other} do
      community = joined_community_fixture(other)
      posts = for _ <- 1..3, do: post_fixture(other, [community.id])
      phone = SyncClient.pull(SyncClient.new(author))
      refute SyncClient.rows(phone, "app_posts") |> Enum.any?(&(&1["author_id"] == other.user.id))

      {:ok, _membership} = Memberships.join(author, community.id)
      phone = SyncClient.backfill(phone, community.id, limit: 2)

      assert length(phone.last_pull) == 2

      assert Enum.sort(Enum.map(SyncClient.rows(phone, "app_posts"), & &1["id"])) ==
               Enum.sort(Enum.map(posts, & &1.id))

      assert length(SyncClient.rows(phone, "app_posts_communities")) == 3
    end

    test "refuses a non-member", %{author: author, other: other} do
      community = joined_community_fixture(other)
      assert Sync.backfill(author, community.id, %{}) == {:error, :not_a_member}
    end
  end
end
