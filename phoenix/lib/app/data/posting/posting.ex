defmodule App.Data.Posting do
  @moduledoc """
  Writing a post, adding it to and removing it from communities, deleting
  it, and reading a feed, over the `App.Data.Posts` and
  `App.Data.PostsCommunities` contexts.

  Only the owner adds their post to a community or removes it, and only
  to a community they are a member of. Adding a post back revives the row
  removed earlier. Deleting a post is a soft delete with no cascade: its
  community rows stay, hidden because every read joins through the post.

  Every write records an `App.Data.PostEvent` in the post's event log,
  see "Events" in `docs/design/offline-sync.md`.
  """

  alias App.Data.Memberships
  alias App.Data.Post
  alias App.Data.PostCommunity
  alias App.Data.PostEvent
  alias App.Data.PostEvents
  alias App.Data.Posts
  alias App.Data.PostsCommunities
  alias App.Data.Sync.Merge
  alias App.Web.Scope

  @event_fields %{create: ["body"], update: ["body", "deleted_at"]}

  @doc """
  Writes a post by the scope's user and adds it to the communities, in
  one transaction. Returns `{:error, :not_a_member}` for a community the
  user is not a member of, and the changeset error for a bad post.
  """
  def write(%Scope{} = scope, params, community_ids \\ []) do
    App.Repo.transact(fn -> create_in_communities(scope, params, community_ids) end)
  end

  defp create_in_communities(%Scope{user: user} = scope, params, community_ids) do
    with {:ok, post} <- Posts.create(params, owner_id: user.id),
         {:ok, _event} <- record(scope, post, :create, 0, %{"body" => post.body}, %{}),
         :ok <- add_to_communities(scope, post, community_ids) do
      {:ok, post}
    end
  end

  defp add_to_communities(scope, post, community_ids) do
    Enum.reduce_while(community_ids, :ok, fn community_id, :ok ->
      case add_to_community(scope, post, community_id) do
        {:ok, _row} -> {:cont, :ok}
        error -> {:halt, error}
      end
    end)
  end

  @doc """
  Adds the post to the community, or adds it back. Owner only.
  """
  def add_to_community(
        %Scope{user: %{id: owner_id}} = scope,
        %Post{owner_id: owner_id} = post,
        community_id
      ) do
    case Memberships.membership(scope, community_id) do
      nil ->
        {:error, :not_a_member}

      membership ->
        case PostsCommunities.get_by([post_id: post.id, community_id: community_id],
               include_deleted: true
             ) do
          nil ->
            PostsCommunities.create(%{
              post_id: post.id,
              community_id: community_id,
              users_communities_id: membership.id
            })

          %PostCommunity{deleted_at: nil} = row ->
            {:ok, row}

          %PostCommunity{} = row ->
            PostsCommunities.update(row, %{users_communities_id: membership.id},
              changesets: &PostCommunity.revive_changeset/3
            )
        end
    end
  end

  def add_to_community(_scope, _post, _community_id), do: {:error, :not_owner}

  @doc """
  Removes the post from the community. Owner only.
  """
  def remove_from_community(
        %Scope{user: %{id: owner_id} = user},
        %Post{owner_id: owner_id} = post,
        community_id
      ) do
    case PostsCommunities.get_by(post_id: post.id, community_id: community_id) do
      nil ->
        {:error, :not_found}

      row ->
        PostsCommunities.delete(row,
          deleted_by_id: user.id,
          changesets: &PostCommunity.remove_changeset/3
        )
    end
  end

  def remove_from_community(_scope, _post, _community_id), do: {:error, :not_owner}

  @doc """
  Deletes the post. Owner only.
  """
  def delete(%Scope{user: %{id: owner_id} = user} = scope, %Post{owner_id: owner_id} = post) do
    App.Repo.transact(fn ->
      with {:ok, deleted} <- Posts.delete(post, deleted_by_id: user.id),
           {:ok, _event} <-
             record(
               scope,
               deleted,
               :update,
               post.version,
               %{"deleted_at" => Merge.wire(deleted.deleted_at)},
               %{}
             ) do
        {:ok, deleted}
      end
    end)
  end

  def delete(_scope, _post), do: {:error, :not_owner}

  @doc """
  Applies an event from a device, checked for shape by `App.Data.Sync`.
  Returns `{:ok, event, post}`, with any conflicts in `event.conflicts`,
  or `{:error, reason, post_or_nil}`.
  """
  def apply_event(%Scope{user: user} = scope, %{id: id} = event) do
    case PostEvents.get(id) do
      nil ->
        apply_new_event(scope, event)

      %PostEvent{user_id: user_id} = recorded when user_id == user.id ->
        {:ok, recorded, current(recorded.post_id)}

      %PostEvent{} ->
        {:error, "event_id_taken", nil}
    end
  end

  defp apply_new_event(scope, %{type: type, changes: changes} = event) do
    case Map.keys(changes) -- @event_fields[type] do
      [] -> apply_checked_event(scope, event)
      fields -> {:error, "not_editable: " <> Enum.join(fields, ", "), current(event.row_id)}
    end
  end

  defp apply_checked_event(%Scope{user: user} = scope, %{type: :create} = event) do
    case current(event.row_id) do
      nil ->
        changeset = Posts.change(%Post{id: event.row_id}, event.changes, owner_id: user.id)

        with {:ok, post} <- insert(changeset),
             {:ok, recorded} <- record(scope, post, :create, 0, event.changes, %{}, event.id) do
          {:ok, recorded, post}
        end

      post ->
        {:error, "exists", post}
    end
  end

  defp apply_checked_event(%Scope{user: %{id: user_id}} = scope, %{type: :update} = event) do
    case current(event.row_id) do
      nil ->
        {:error, "unknown_post", nil}

      %Post{owner_id: ^user_id} = post ->
        values = Merge.values(post, @event_fields.update)

        {applied, conflicts} =
          Merge.merge(values, post.version, event.base_version, event.base, event.changes)

        with {:ok, post} <- update(post, applied, user_id),
             {:ok, recorded} <- record(scope, post, :update, event.base_version, applied, conflicts, event.id) do
          {:ok, recorded, post}
        end

      post ->
        {:error, "not_owner", post}
    end
  end

  defp current(post_id), do: Posts.get(post_id, include_deleted: true)

  defp insert(changeset) do
    case App.Repo.insert(changeset) do
      {:ok, post} -> {:ok, post}
      {:error, changeset} -> {:error, invalid(changeset), nil}
    end
  end

  defp update(post, applied, _user_id) when applied == %{}, do: {:ok, post}

  defp update(post, applied, user_id) do
    case Posts.update(post, applied, changesets: &Post.event_changeset/3, deleted_by_id: user_id) do
      {:ok, post} -> {:ok, post}
      {:error, changeset} -> {:error, invalid(changeset), post}
    end
  end

  defp invalid(changeset) do
    "invalid: " <>
      Enum.map_join(changeset.errors, ", ", fn {field, {message, _}} -> "#{field} #{message}" end)
  end

  defp record(%Scope{user: user}, post, type, base_version, changes, conflicts, id \\ nil) do
    PostEvents.create(%{
      id: id,
      post_id: post.id,
      user_id: user.id,
      type: type,
      base_version: base_version,
      version: post.version,
      changes: changes,
      conflicts: conflicts
    })
  end

  @doc """
  The posts the scope reads, newest first, with their owners: inside a
  community, the community's feed; otherwise everything visible to the
  user across communities.
  """
  def feed(%Scope{community: %{id: community_id}}) do
    Posts.list(filters: [in_community: community_id], preloads: [:owner])
  end

  def feed(%Scope{} = scope) do
    Posts.list(filters: [visible_to: scope], preloads: [:owner])
  end

  @doc """
  The live community rows of the post.
  """
  def communities_of(_scope, %Post{id: post_id}) do
    PostsCommunities.list(filters: [post_id: post_id])
  end
end
