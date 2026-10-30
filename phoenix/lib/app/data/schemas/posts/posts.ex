defmodule App.Data.Posts do
  @moduledoc """
  The data context over `App.Data.Post`, with the two visibility
  filters of `docs/design/multi-tenancy.md`:

    * `in_community: community_id` - the posts in the community, for a
      request already scoped to one, where membership was checked.
    * `visible_to: scope` - across communities: the posts in a community
      the scope's user is a live member of, or owned by the user. A
      post in no community is visible only to its owner.

  Both see only live `app_posts_communities` rows. Writing, adding to and
  removing from communities are in `App.Data.Posting`.
  """

  use App.Data.Context, schema: App.Data.Post, sorts: [desc: :inserted_at]

  import Ecto.Query, only: [from: 2]

  alias App.Data.PostCommunity
  alias App.Data.UserCommunity
  alias App.Web.Scope

  def filter(query, :in_community, community_id) do
    in_community =
      from(pc in PostCommunity,
        where: pc.community_id == ^community_id and is_nil(pc.deleted_at),
        select: pc.post_id
      )

    Ecto.Query.where(query, [post], post.id in subquery(in_community))
  end

  def filter(query, :visible_to, %Scope{user: user}) do
    memberships =
      from(uc in UserCommunity,
        where: uc.user_id == ^user.id and is_nil(uc.deleted_at),
        select: uc.community_id
      )

    in_my_communities =
      from(pc in PostCommunity,
        where: pc.community_id in subquery(memberships) and is_nil(pc.deleted_at),
        select: pc.post_id
      )

    Ecto.Query.where(
      query,
      [post],
      post.owner_id == ^user.id or post.id in subquery(in_my_communities)
    )
  end

  def filter(query, key, value), do: super(query, key, value)
end
