defmodule App.Data.Memberships do
  @moduledoc """
  Joining, leaving and rejoining a community, over the
  `App.Data.UsersCommunities` context. A membership is a `users_communities`
  row of the scope's user.

  Leaving soft-deletes the row with one `deleted_at` value, computed once
  here, and cascades to the member's own posts in the community: every
  live `posts_communities` row added under the membership gets the same
  `deleted_at`. Rejoining while the row still exists clears `deleted_at`
  on that same row, so its id is stable, and revives exactly the rows
  that left with it: those under the membership whose `deleted_at` equals
  the membership's. A row the author removed on purpose was deleted at
  another moment and stays deleted. Once the purge has removed the
  membership, rejoining starts fresh.
  """

  import Ecto.Query, only: [from: 2]

  alias App.Data.Communities
  alias App.Data.PostCommunity
  alias App.Data.PostsCommunities
  alias App.Data.UserCommunity
  alias App.Data.UsersCommunities
  alias App.Web.Scope

  @doc """
  The live membership of the scope's user in the community, or nil.
  """
  def membership(%Scope{user: user}, community_id) do
    UsersCommunities.get_by(user_id: user.id, community_id: community_id)
  end

  @doc """
  Whether the scope's user is a member of the community.
  """
  def member?(scope, community_id), do: membership(scope, community_id) != nil

  @doc """
  Joins the community, or rejoins it by reviving the membership left
  behind and what left with it. Returns the live membership either way.
  """
  def join(%Scope{user: user}, community_id) do
    community = Communities.get(community_id)

    case UsersCommunities.get_by([user_id: user.id, community_id: community_id], include_deleted: true) do
      _membership when community == nil ->
        {:error, :unknown_community}

      nil ->
        UsersCommunities.create(%{user_id: user.id, community_id: community_id})

      %UserCommunity{deleted_at: nil} = membership ->
        {:ok, membership}

      %UserCommunity{} = membership ->
        App.Repo.transact(fn -> rejoin(membership) end)
    end
  end

  defp rejoin(%UserCommunity{deleted_at: left_at} = membership) do
    with {:ok, membership} <-
           UsersCommunities.update(membership, %{}, changesets: &UserCommunity.rejoin_changeset/3) do
      revive_rows(membership, left_at)
      {:ok, membership}
    end
  end

  @doc """
  Leaves the community, taking the member's own posts there along.
  Returns `{:error, :not_found}` without a live membership.
  """
  def leave(%Scope{user: user} = scope, community_id) do
    case membership(scope, community_id) do
      nil ->
        {:error, :not_found}

      membership ->
        App.Repo.transact(fn -> leave_with_posts(membership, user.id) end)
    end
  end

  defp leave_with_posts(membership, user_id) do
    deleted_at = DateTime.utc_now()

    with {:ok, membership} <-
           UsersCommunities.delete(membership,
             deleted_at: deleted_at,
             deleted_by_id: user_id,
             changesets: &UserCommunity.leave_changeset/3
           ) do
      cascade_rows(membership, deleted_at)
      {:ok, membership}
    end
  end

  defp cascade_rows(%UserCommunity{id: id, deleted_by_id: deleted_by_id}, deleted_at) do
    PostsCommunities.delete_many(
      filters: [users_communities_id: id],
      deleted_at: deleted_at,
      deleted_by_id: deleted_by_id
    )
  end

  defp revive_rows(%UserCommunity{id: id}, left_at) do
    App.Repo.update_all(
      from(pc in PostCommunity, where: pc.users_communities_id == ^id and pc.deleted_at == ^left_at),
      set: [deleted_at: nil, deleted_by_id: nil, last_synced_at: DateTime.utc_now()],
      inc: [version: 1]
    )
  end
end
