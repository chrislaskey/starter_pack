defmodule App.Data.Posting do
  @moduledoc """
  Writing a post, adding it to and removing it from communities, deleting
  it, and reading a feed, over the `App.Data.Posts` and
  `App.Data.PostsCommunities` contexts.

  Only the author adds their post to a community or removes it, and only
  to a community they are a member of. Adding a post back revives the row
  removed earlier. Deleting a post is a soft delete with no cascade: its
  community rows stay, hidden because every read joins through the post.
  """

  alias App.Data.Memberships
  alias App.Data.Post
  alias App.Data.PostCommunity
  alias App.Data.Posts
  alias App.Data.PostsCommunities
  alias App.Web.Scope

  @doc """
  Writes a post by the scope's user and adds it to the communities, in
  one transaction. Returns `{:error, :not_a_member}` for a community the
  user is not a member of, and the changeset error for a bad post.
  """
  def write(%Scope{} = scope, params, community_ids \\ []) do
    App.Repo.transact(fn -> create_in_communities(scope, params, community_ids) end)
  end

  defp create_in_communities(%Scope{user: user} = scope, params, community_ids) do
    with {:ok, post} <- Posts.create(params, author_id: user.id),
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
  Adds the post to the community, or adds it back. Author only.
  """
  def add_to_community(
        %Scope{user: %{id: author_id}} = scope,
        %Post{author_id: author_id} = post,
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

  def add_to_community(_scope, _post, _community_id), do: {:error, :not_author}

  @doc """
  Removes the post from the community. Author only.
  """
  def remove_from_community(
        %Scope{user: %{id: author_id} = user},
        %Post{author_id: author_id} = post,
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

  def remove_from_community(_scope, _post, _community_id), do: {:error, :not_author}

  @doc """
  Deletes the post. Author only.
  """
  def delete(%Scope{user: %{id: author_id} = user}, %Post{author_id: author_id} = post) do
    Posts.delete(post, deleted_by_id: user.id)
  end

  def delete(_scope, _post), do: {:error, :not_author}

  @doc """
  The posts the scope reads, newest first, with their authors: inside a
  community, the community's feed; otherwise everything visible to the
  user across communities.
  """
  def feed(%Scope{community: %{id: community_id}}) do
    Posts.list(filters: [in_community: community_id], preloads: [:author])
  end

  def feed(%Scope{} = scope) do
    Posts.list(filters: [visible_to: scope], preloads: [:author])
  end

  @doc """
  The live community rows of the post.
  """
  def communities_of(_scope, %Post{id: post_id}) do
    PostsCommunities.list(filters: [post_id: post_id])
  end
end
