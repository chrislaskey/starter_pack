defmodule App.Data.Sync do
  @moduledoc """
  The sync server of `docs/design/offline-sync.md`: pull, push and
  backfill for the scope's user, over the data contexts. `App.Web.SyncController` serves it over HTTP.

  Rows travel as `%{"table" => name, "row" => columns}`, the columns being
  the table's, with datetimes as UTC ISO 8601 text with six fractional
  digits. The synced tables are `tables/0`, parents first.

  ## Pull

  The rows visible to the user with `last_synced_at` later than the
  cursor, every table merged and ordered by `(last_synced_at, id)`,
  paged. A `posts_communities` row carries its post and the post's author,
  regardless of their `last_synced_at`. The request's `cursor` is what the
  client stored after its last sync, pulled from an overlap buffer
  earlier so a write that committed late is not missed; `after` is the
  exact page token of the previous response, for the following pages. The
  response has `rows`, `next`, the token of the next page or nil, and
  `cursor`, what to store once `next` is nil.

  ## Push

  One transaction per batch, rows applied parents first. Every datetime
  is checked for the exact format, `updated_at` and `deleted_at` are
  bounded by the server clock, and every row is authorized against the
  scope. Posts and communities are last write wins on `updated_at`.
  Memberships and `posts_communities` rows are server-wins: they go
  through `App.Data.Memberships` and `App.Data.Posting`, and the server's
  version comes back. The response lists `accepted` rows with their new
  `last_synced_at`, and `rejected` rows with a reason and `server`, the
  server's version of the row or nil, which the client applies.

  ## Backfill

  Every live `posts_communities` row of a community the user is a member
  of, each carrying its post, paged like a pull. A client calls it once
  when it sees a new membership of its own user.
  """

  alias App.Data.AppUser
  alias App.Data.AppUsers
  alias App.Data.Communities
  alias App.Data.Community
  alias App.Data.Memberships
  alias App.Data.Post
  alias App.Data.PostCommunity
  alias App.Data.Posting
  alias App.Data.Posts
  alias App.Data.PostsCommunities
  alias App.Data.UserCommunity
  alias App.Data.UsersCommunities
  alias App.Web.Scope

  @overlap_seconds 60
  @default_limit 100
  @max_limit 500
  @epoch ~U[1970-01-01 00:00:00.000000Z]
  @datetime ~r/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{6}Z$/

  # Parents first, which is the push order.
  @tables [
    {"app_users", AppUser, AppUsers},
    {"app_communities", Community, Communities},
    {"app_users_communities", UserCommunity, UsersCommunities},
    {"app_posts", Post, Posts},
    {"app_posts_communities", PostCommunity, PostsCommunities}
  ]

  @doc """
  The synced tables as `{name, schema, context}`, parents first.
  """
  def tables, do: @tables

  @doc """
  The synced schemas, parents first.
  """
  def schemas, do: Enum.map(@tables, &elem(&1, 1))

  @doc """
  How far before the stored cursor a pull starts, in seconds.
  """
  def overlap_seconds, do: @overlap_seconds

  ## Pull

  def pull(%Scope{} = scope, params) do
    horizon = App.Data.Purge.horizon()

    if resync?(params, horizon) do
      {:ok, %{"rows" => [], "next" => nil, "cursor" => nil, "purge_horizon" => horizon, "resync" => true}}
    else
      page = scope |> pull_page(params) |> Map.merge(%{"purge_horizon" => horizon, "resync" => false})
      {:ok, page}
    end
  rescue
    ArgumentError -> {:error, :bad_cursor}
  end

  # A client whose cursor is older than the purge horizon may have missed
  # a tombstone, so it drops what it has and pulls from nothing.
  defp resync?(%{"cursor" => %{"last_synced_at" => at}}, %DateTime{} = horizon) when is_binary(at) do
    DateTime.compare(parse_datetime!(at), horizon) == :lt
  end

  defp resync?(_params, _horizon), do: false

  defp pull_page(scope, params) do
    limit = limit(params)
    cursor = request_cursor(params)

    rows =
      for {table, _schema, context} <- @tables,
          row <-
            context.list(
              filters: [visible_to: scope, synced_after: cursor],
              include_deleted: true,
              order_by: [:last_synced_at, :id],
              limit: limit + 1
            ),
          do: {table, row}

    page(rows, limit, params["cursor"])
  end

  ## Backfill

  def backfill(%Scope{} = scope, community_id, params) do
    if Memberships.member?(scope, community_id) do
      limit = limit(params)

      rows =
        for row <-
              PostsCommunities.list(
                filters: [community_id: community_id, synced_after: page_cursor(params)],
                order_by: [:last_synced_at, :id],
                limit: limit + 1
              ),
            do: {"app_posts_communities", row}

      {:ok, page(rows, limit, nil)}
    else
      {:error, :not_a_member}
    end
  rescue
    ArgumentError -> {:error, :bad_cursor}
  end

  # One page of `{table, row}` pairs: the first `limit` in cursor order,
  # carried rows included, with the token of the next page.
  defp page(rows, limit, stored_cursor) do
    sorted = Enum.sort_by(rows, fn {_table, row} -> sort_key(row) end)
    {page, rest} = Enum.split(sorted, limit)

    cursor =
      case List.last(page) do
        nil -> stored_cursor
        {_table, row} -> %{"last_synced_at" => row.last_synced_at, "id" => row.id}
      end

    %{
      "rows" => page |> carry() |> Enum.map(&encode/1),
      "next" => if(rest != [], do: cursor),
      "cursor" => cursor
    }
  end

  defp sort_key(row), do: {DateTime.to_unix(row.last_synced_at, :microsecond), row.id}

  # Every live `posts_communities` row carries its post and the post's
  # author, placed before it so a client with foreign keys can apply in
  # order. A removed row carries nothing: a client that held the post
  # prunes it, and one that never did must not receive it now.
  defp carry(page) do
    post_ids = for {"app_posts_communities", %{deleted_at: nil} = row} <- page, uniq: true, do: row.post_id

    posts =
      [filters: [id: post_ids], include_deleted: true]
      |> Posts.list()
      |> Map.new(&{&1.id, &1})

    author_ids = posts |> Map.values() |> Enum.map(& &1.author_id) |> Enum.uniq()

    authors =
      [filters: [id: author_ids], include_deleted: true]
      |> AppUsers.list()
      |> Map.new(&{&1.id, &1})

    page
    |> Enum.flat_map(fn
      {"app_posts_communities", %{deleted_at: nil} = row} = pair ->
        post = Map.fetch!(posts, row.post_id)
        [{"app_users", Map.fetch!(authors, post.author_id)}, {"app_posts", post}, pair]

      pair ->
        [pair]
    end)
    |> Enum.uniq_by(fn {table, row} -> {table, row.id} end)
  end

  defp limit(params) do
    case params["limit"] do
      count when is_integer(count) and count > 0 -> min(count, @max_limit)
      _ -> @default_limit
    end
  end

  # The exact page token when there is one, else the stored cursor from
  # the overlap buffer earlier, else the beginning.
  defp request_cursor(%{"after" => %{} = token}), do: page_cursor(%{"after" => token})

  defp request_cursor(%{"cursor" => %{"last_synced_at" => at}}) when is_binary(at) do
    {DateTime.add(parse_datetime!(at), -@overlap_seconds, :second), nil}
  end

  defp request_cursor(_params), do: {@epoch, nil}

  defp page_cursor(%{"after" => %{"last_synced_at" => at, "id" => id}})
       when is_binary(at) and is_binary(id),
       do: {parse_datetime!(at), id}

  defp page_cursor(_params), do: {@epoch, nil}

  defp parse_datetime!(text) do
    case DateTime.from_iso8601(text) do
      {:ok, datetime, 0} -> datetime
      _ -> raise ArgumentError, "not a UTC datetime: #{inspect(text)}"
    end
  end

  ## Push

  def push(%Scope{} = scope, rows) when is_list(rows) do
    {:ok, results} =
      App.Repo.transact(fn -> {:ok, Enum.map(sort_parents_first(rows), &apply_row(scope, &1))} end)

    %{
      "accepted" =>
        for(
          {:accepted, table, id, at} <- results,
          do: %{"table" => table, "id" => id, "last_synced_at" => at}
        ),
      "rejected" =>
        for {:rejected, table, id, reason, server} <- results do
          %{"table" => table, "id" => id, "reason" => reason, "server" => server}
        end
    }
  end

  defp sort_parents_first(rows) do
    order = @tables |> Enum.map(&elem(&1, 0)) |> Enum.with_index() |> Map.new()
    Enum.sort_by(rows, &Map.get(order, table_of(&1), map_size(order)))
  end

  defp apply_row(scope, %{"table" => table, "row" => %{"id" => id} = row}) do
    with {:ok, schema, context} <- lookup(table),
         {:ok, incoming} <- decode(schema, row) do
      apply_row(scope, table, context, incoming)
    else
      {:error, reason} -> {:rejected, table, id, reason, nil}
    end
  end

  defp apply_row(_scope, other) do
    {:rejected, table_of(other), id_of(other), "malformed", nil}
  end

  defp table_of(%{"table" => table}) when is_binary(table), do: table
  defp table_of(_other), do: nil

  defp id_of(%{"row" => %{"id" => id}}) when is_binary(id), do: id
  defp id_of(_other), do: nil

  defp lookup(table) do
    case List.keyfind(@tables, table, 0) do
      {^table, schema, context} -> {:ok, schema, context}
      nil -> {:error, "unknown_table"}
    end
  end

  # The row as the schema's struct: every datetime in the exact format,
  # every column cast, the schema's own validations applied.
  defp decode(schema, row) do
    with :ok <- check_datetimes(schema, row) do
      changeset =
        schema
        |> struct()
        |> Ecto.Changeset.cast(row, schema.__schema__(:fields) -- [:last_synced_at])
        |> schema.changeset(row)
        |> Ecto.Changeset.validate_required([:id, :updated_at])

      if changeset.valid? do
        {:ok, Ecto.Changeset.apply_changes(changeset)}
      else
        {:error,
         "invalid: " <>
           Enum.map_join(changeset.errors, ", ", fn {field, {message, _}} -> "#{field} #{message}" end)}
      end
    end
  end

  defp check_datetimes(schema, row) do
    bad =
      for field <- schema.__schema__(:fields),
          schema.__schema__(:type, field) == :utc_datetime_usec,
          value = row[Atom.to_string(field)],
          value != nil,
          not (is_binary(value) and Regex.match?(@datetime, value)),
          do: field

    case bad do
      [] -> :ok
      fields -> {:error, "bad_datetime: " <> Enum.join(fields, ", ")}
    end
  end

  # App users come from registration and are never accepted from a client.
  defp apply_row(_scope, "app_users", context, incoming) do
    {:rejected, "app_users", incoming.id, "server_wins", server_version(context, incoming.id)}
  end

  # A client creates a community, as any user may,
  # and never changes one: the web app has no edit or delete either, and
  # who may will be a role, later. Server-wins until then.
  defp apply_row(scope, "app_communities", context, incoming) do
    case context.get(incoming.id, include_deleted: true) do
      nil ->
        write(scope, "app_communities", Community, nil, incoming)

      existing ->
        {:rejected, "app_communities", incoming.id, "server_wins", encode({"app_communities", existing})}
    end
  end

  defp apply_row(%Scope{user: user} = scope, "app_posts", context, incoming) do
    existing = context.get(incoming.id, include_deleted: true)

    if incoming.author_id == user.id and (existing == nil or existing.author_id == user.id) do
      write(scope, "app_posts", Post, existing, incoming)
    else
      {:rejected, "app_posts", incoming.id, "not_author", existing && encode({"app_posts", existing})}
    end
  end

  # Server-wins: the client's row is an intent, applied through the flow,
  # and the server's row comes back for the client to apply.
  defp apply_row(%Scope{user: user} = scope, "app_users_communities", context, incoming) do
    result =
      cond do
        incoming.user_id != user.id -> {:error, "not_own_membership"}
        incoming.deleted_at == nil -> Memberships.join(scope, incoming.community_id)
        true -> leave(scope, incoming.community_id)
      end

    server_wins("app_users_communities", context, incoming, result)
  end

  defp apply_row(scope, "app_posts_communities", context, incoming) do
    result =
      case Posts.get(incoming.post_id, include_deleted: true) do
        nil -> {:error, "unknown_post"}
        post when incoming.deleted_at == nil -> Posting.add_to_community(scope, post, incoming.community_id)
        post -> remove(scope, post, incoming.community_id)
      end

    server_wins("app_posts_communities", context, incoming, result)
  end

  defp leave(scope, community_id) do
    case Memberships.leave(scope, community_id) do
      {:error, :not_found} -> {:ok, nil}
      result -> result
    end
  end

  defp remove(scope, post, community_id) do
    case Posting.remove_from_community(scope, post, community_id) do
      {:error, :not_found} -> {:ok, nil}
      result -> result
    end
  end

  defp server_wins(table, context, incoming, result) do
    case result do
      {:ok, nil} ->
        {:rejected, table, incoming.id, "server_wins", server_version(context, incoming.id)}

      {:ok, row} ->
        {:rejected, table, incoming.id, "server_wins", encode({table, row})}

      {:error, reason} ->
        {:rejected, table, incoming.id, to_string(reason), server_version(context, incoming.id)}
    end
  end

  defp server_version(context, id) do
    case context.get(id, include_deleted: true) do
      nil -> nil
      row -> encode({context.schema().__schema__(:source), row})
    end
  end

  # Last write wins on `updated_at`, bounded by the server clock, written
  # with the server's `last_synced_at`. The same `updated_at` again is a
  # retried push, accepted as it stands.
  defp write(%Scope{user: user}, table, schema, existing, incoming) do
    now = DateTime.utc_now()
    incoming = bound(incoming, now, user)
    order = existing && DateTime.compare(incoming.updated_at, existing.updated_at)

    cond do
      order == :lt ->
        {:rejected, table, incoming.id, "stale", encode({table, existing})}

      order == :eq ->
        {:accepted, table, incoming.id, existing.last_synced_at}

      true ->
        write_row(table, schema, existing, incoming, now)
    end
  end

  defp write_row(table, schema, existing, incoming, now) do
    fields = schema.__schema__(:fields)

    values =
      incoming
      |> Map.take(fields)
      |> Map.put(:last_synced_at, now)
      |> Map.update!(:inserted_at, &(&1 || (existing && existing.inserted_at) || now))

    App.Repo.insert_all(schema, [values],
      on_conflict: {:replace, fields -- [:id, :inserted_at]},
      conflict_target: [:id]
    )

    {:accepted, table, incoming.id, now}
  end

  # No datetime from a client is later than now, and a client deletes
  # only as itself.
  defp bound(incoming, now, user) do
    incoming
    |> Map.update!(:updated_at, &earliest(&1, now))
    |> Map.update!(:inserted_at, &earliest(&1, now))
    |> Map.update!(:deleted_at, &earliest(&1, now))
    |> Map.update!(:deleted_by_id, &if(incoming.deleted_at, do: user.id, else: &1 && nil))
  end

  defp earliest(nil, _now), do: nil
  defp earliest(datetime, now), do: if(DateTime.compare(datetime, now) == :gt, do: now, else: datetime)

  ## Rows on the wire

  @doc """
  A `{table, struct}` pair as `%{"table" => name, "row" => columns}`.
  """
  def encode({"app_posts", %Post{deleted_at: %DateTime{}} = post}) do
    # A tombstone travels without its content
    encode_fields({"app_posts", %{post | body: ""}})
  end

  def encode(pair), do: encode_fields(pair)

  defp encode_fields({table, %schema{} = row}) do
    %{"table" => table, "row" => Map.take(row, schema.__schema__(:fields))}
  end
end
