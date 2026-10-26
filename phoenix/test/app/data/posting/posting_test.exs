defmodule App.Data.PostingTest do
  use App.DataCase

  import App.Data.CommunitiesFixtures
  import App.Data.PostsFixtures
  import App.Data.UsersFixtures

  alias App.Data.Memberships
  alias App.Data.PostCommunity
  alias App.Data.Posting
  alias App.Data.Posts
  alias App.Data.PostsCommunities
  alias App.Web.Scope

  setup do
    author = user_scope_fixture(user_fixture())
    reader = user_scope_fixture(user_fixture())

    [one, two, three] =
      for _ <- 1..3 do
        community = joined_community_fixture(author)
        {:ok, _membership} = Memberships.join(reader, community.id)
        community
      end

    %{author: author, reader: reader, one: one, two: two, three: three}
  end

  defp feed_ids(scope, community \\ nil) do
    scope = if community, do: Scope.put_community(scope, community), else: scope
    scope |> Posting.feed() |> Enum.map(& &1.id)
  end

  describe "write/3" do
    test "puts the post in the chosen communities, with their authors preloaded in the feed",
         %{author: author, reader: reader, one: one, two: two, three: three} do
      post = post_fixture(author, [one.id, two.id])

      assert feed_ids(reader, one) == [post.id]
      assert feed_ids(reader, two) == [post.id]
      assert feed_ids(reader, three) == []
      assert [%{author: %{id: author_id}}] = Posting.feed(Scope.put_community(reader, one))
      assert author_id == author.user.id
    end

    test "refuses a community the author is not a member of", %{author: author} do
      other = community_fixture()

      assert {:error, :not_a_member} = Posting.write(author, %{body: "hello"}, [other.id])
      assert Posts.list() == []
    end

    test "validates the post", %{author: author} do
      assert {:error, changeset} = Posting.write(author, %{body: ""})
      assert %{body: ["can't be blank"]} = errors_on(changeset)
    end
  end

  describe "feed/1 across communities" do
    test "shows posts in shared communities and the user's own, and nothing else",
         %{author: author, reader: reader, one: one} do
      shared = post_fixture(author, [one.id])
      alone = post_fixture(author, [])

      stranger = user_scope_fixture(user_fixture())
      private = joined_community_fixture(stranger)
      unseen = post_fixture(stranger, [private.id])

      assert feed_ids(reader) == [shared.id]
      assert feed_ids(author) == [alone.id, shared.id]
      assert feed_ids(stranger) == [unseen.id]
    end
  end

  describe "remove_from_community/3 and add_to_community/3" do
    test "removing a post from one community leaves it in the others",
         %{author: author, reader: reader, one: one, two: two} do
      post = post_fixture(author, [one.id, two.id])

      assert {:ok, %PostCommunity{deleted_at: %DateTime{}}} =
               Posting.remove_from_community(author, post, one.id)

      assert feed_ids(reader, one) == []
      assert feed_ids(reader, two) == [post.id]
    end

    test "adding a post back revives the same row", %{author: author, reader: reader, one: one} do
      post = post_fixture(author, [one.id])
      [%{id: row_id}] = Posting.communities_of(author, post)
      {:ok, _row} = Posting.remove_from_community(author, post, one.id)

      assert {:ok, %PostCommunity{id: ^row_id, deleted_at: nil}} =
               Posting.add_to_community(author, post, one.id)

      assert feed_ids(reader, one) == [post.id]
    end

    test "only the author adds or removes", %{author: author, reader: reader, one: one, two: two} do
      post = post_fixture(author, [one.id])

      assert {:error, :not_author} = Posting.add_to_community(reader, post, two.id)
      assert {:error, :not_author} = Posting.remove_from_community(reader, post, one.id)
      assert {:error, :not_author} = Posting.delete(reader, post)
    end
  end

  describe "leaving and rejoining" do
    test "leaving removes the author's posts from that community and rejoining brings them back",
         %{author: author, reader: reader, one: one, two: two} do
      post = post_fixture(author, [one.id, two.id])
      other = post_fixture(reader, [one.id])

      {:ok, membership} = Memberships.leave(author, one.id)

      assert feed_ids(reader, one) == [other.id]
      assert feed_ids(reader, two) == [post.id]

      [row] =
        PostsCommunities.list(
          filters: [post_id: post.id, community_id: one.id],
          include_deleted: true
        )

      assert row.deleted_at == membership.deleted_at

      {:ok, _membership} = Memberships.join(author, one.id)
      assert feed_ids(reader, one) == [other.id, post.id]
    end

    test "a row the author removed on purpose does not come back on rejoin",
         %{author: author, reader: reader, one: one} do
      kept = post_fixture(author, [one.id])
      removed = post_fixture(author, [one.id])
      {:ok, _row} = Posting.remove_from_community(author, removed, one.id)

      {:ok, _membership} = Memberships.leave(author, one.id)
      {:ok, _membership} = Memberships.join(author, one.id)

      assert feed_ids(reader, one) == [kept.id]
    end
  end

  describe "delete/2" do
    test "hides the post from every feed with no cascade of its own",
         %{author: author, reader: reader, one: one} do
      post = post_fixture(author, [one.id])

      assert {:ok, %{deleted_at: %DateTime{}}} = Posting.delete(author, post)

      assert feed_ids(reader, one) == []
      assert feed_ids(reader) == []
      assert feed_ids(author) == []
      assert [%PostCommunity{deleted_at: nil}] = PostsCommunities.list(filters: [post_id: post.id])
    end
  end
end
