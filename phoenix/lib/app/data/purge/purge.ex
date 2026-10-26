defmodule App.Data.Purge do
  @moduledoc """
  The purge job: hard-deletes what was soft-deleted more than the
  retention window ago, see "Leak 3" in `docs/design/offline-sync.md`.

  A synced row goes once it has been a tombstone for the whole window:
  `deleted_at` and `last_synced_at` both older than the horizon, so every
  client that pulled in the window has seen it. Children go before
  parents, and users after every `app_` table. The run then records the
  horizon in `app_purges`; `App.Data.Sync` tells a client whose cursor is
  older to resync from nothing, since it may have missed a tombstone.

  A purged parent takes its children through the foreign keys, whatever
  their age: a deleted post's community rows, a left membership's rows, a
  deleted user's posts. That is safe because every read joins through
  the parent, so nothing could reach those children once the parent's
  tombstone, which every client in the window received, was in place.

  Run it from `mix app.purge` or `App.Release.purge/0`, on a schedule,
  such as a daily cron job on the server.
  """

  import Ecto.Query, only: [from: 2]

  alias App.Data.AppUsers
  alias App.Data.Communities
  alias App.Data.Posts
  alias App.Data.PostsCommunities
  alias App.Data.Users
  alias App.Data.UsersCommunities

  @retention_days 90

  # Children first
  @synced_contexts [PostsCommunities, Posts, UsersCommunities, Communities, AppUsers]

  def retention_days, do: @retention_days

  @doc """
  Purges every table and records the horizon. Returns the horizon and how
  many rows went per table. `now:` is for tests.
  """
  def run(opts \\ []) do
    now = Keyword.get(opts, :now, DateTime.utc_now())
    horizon = DateTime.add(now, -@retention_days, :day)

    {:ok, counts} = App.Repo.transact(fn -> {:ok, purge_tables(horizon)} end)

    %{horizon: horizon, counts: counts}
  end

  defp purge_tables(horizon) do
    synced =
      for context <- @synced_contexts, into: %{} do
        {count, _} =
          context.delete_many(
            filters: [deleted_at_before: horizon, last_synced_at_before: horizon],
            hard: true
          )

        {context.schema().__schema__(:source), count}
      end

    # Tokens go with the user through the foreign key
    {users, _} = Users.delete_many(filters: [deleted_at_before: horizon], hard: true)

    App.Repo.insert_all("app_purges", [
      %{id: Ecto.UUID.generate(), horizon: horizon, inserted_at: DateTime.utc_now()}
    ])

    Map.put(synced, "users", users)
  end

  @doc """
  The horizon of the last purge: the oldest `last_synced_at` a client's
  cursor may have without a resync, or nil before the first purge.
  """
  def horizon do
    App.Repo.one(
      from(p in "app_purges",
        order_by: [desc: p.inserted_at],
        limit: 1,
        select: type(p.horizon, :utc_datetime_usec)
      )
    )
  end
end
