defmodule App.Data.AppUsers do
  @moduledoc """
  The data context over `App.Data.AppUser`.

  The `visible_to: scope` filter is what syncs to a user: themselves and
  the members of the communities they are a live member of. The authors
  of visible posts travel with the posts, see `App.Data.Sync`.
  """

  use App.Data.Context, schema: App.Data.AppUser

  import Ecto.Query, only: [from: 2]

  alias App.Data.UserCommunity
  alias App.Web.Scope

  def filter(query, :visible_to, %Scope{user: user}) do
    my_communities =
      from(uc in UserCommunity,
        where: uc.user_id == ^user.id and is_nil(uc.deleted_at),
        select: uc.community_id
      )

    members =
      from(uc in UserCommunity,
        where: uc.community_id in subquery(my_communities) and is_nil(uc.deleted_at),
        select: uc.user_id
      )

    Ecto.Query.where(query, [u], u.id == ^user.id or u.id in subquery(members))
  end

  def filter(query, key, value), do: super(query, key, value)
end
