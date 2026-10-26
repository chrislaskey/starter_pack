defmodule App.SyncClient do
  @moduledoc """
  A simulated offline client for the sync tests: one device of one user,
  as a plain struct. It holds its rows as a client would,
  keyed by table and id, with the values as they come off the wire, and
  calls `App.Data.Sync` directly instead of over HTTP.

      phone = SyncClient.new(scope)
      phone = SyncClient.pull(phone)
      phone = SyncClient.write(phone, "app_posts", %{"body" => "hello", "author_id" => user.id})
      {phone, response} = SyncClient.push(phone)

  `clock_offset` shifts the device's clock, in seconds, for the edits it
  makes.
  """

  alias App.Data.Sync

  defstruct scope: nil, cursor: nil, rows: %{}, outbox: [], clock_offset: 0, last_pull: []

  def new(scope, opts \\ []) do
    %__MODULE__{scope: scope, clock_offset: Keyword.get(opts, :clock_offset, 0)}
  end

  @doc """
  The client's copy of the row, or nil.
  """
  def row(%__MODULE__{rows: rows}, table, id), do: get_in(rows, [table, id])

  @doc """
  The client's rows of the table, in id order.
  """
  def rows(%__MODULE__{rows: rows}, table) do
    rows |> Map.get(table, %{}) |> Map.values() |> Enum.sort_by(& &1["id"])
  end

  @doc """
  Pulls every page from the stored cursor and applies the rows. The pages
  are kept in `last_pull` for assertions.
  """
  def pull(%__MODULE__{} = client, opts \\ []) do
    pull_pages(%{client | last_pull: []}, nil, Keyword.get(opts, :limit, 100))
  end

  defp pull_pages(client, after_token, limit) do
    params = %{"cursor" => client.cursor, "after" => after_token, "limit" => limit}
    {:ok, response} = Sync.pull(client.scope, params)
    response = wire(response)
    client = %{client | last_pull: client.last_pull ++ [response]}

    if response["resync"] do
      # The server purged tombstones this device never saw: start over
      pull_pages(%{client | rows: %{}, cursor: nil}, nil, limit)
    else
      client = Enum.reduce(response["rows"], client, &apply_row(&2, &1))

      case response["next"] do
        nil -> %{client | cursor: response["cursor"]}
        token -> pull_pages(client, token, limit)
      end
    end
  end

  @doc """
  Backfills a community the user joined, applying every page.
  """
  def backfill(%__MODULE__{} = client, community_id, opts \\ []) do
    backfill_pages(%{client | last_pull: []}, community_id, nil, Keyword.get(opts, :limit, 100))
  end

  defp backfill_pages(client, community_id, after_token, limit) do
    params = %{"after" => after_token, "limit" => limit}
    {:ok, response} = Sync.backfill(client.scope, community_id, params)
    response = wire(response)
    client = %{client | last_pull: client.last_pull ++ [response]}
    client = Enum.reduce(response["rows"], client, &apply_row(&2, &1))

    case response["next"] do
      nil -> client
      token -> backfill_pages(client, community_id, token, limit)
    end
  end

  @doc """
  A local write: upserts the row with a client-generated id and
  `updated_at` from the device clock, and queues it for the next push.
  The columns not given keep the client's current values, or nil.
  """
  def write(%__MODULE__{} = client, table, columns, opts \\ []) do
    id = columns["id"] || Ecto.UUID.generate(version: 7)
    now = device_time(client, Keyword.get(opts, :at))
    current = row(client, table, id) || %{"id" => id, "inserted_at" => now}

    row =
      current
      |> Map.merge(columns)
      |> Map.put("updated_at", now)
      |> Map.put_new("deleted_at", nil)
      |> Map.put_new("deleted_by_id", nil)
      |> Map.put_new("last_synced_at", nil)

    client
    |> put_row(table, row)
    |> Map.update!(:outbox, &Enum.uniq(&1 ++ [{table, id}]))
  end

  @doc """
  A local soft delete, queued like a write.
  """
  def delete(%__MODULE__{scope: scope} = client, table, id, opts \\ []) do
    now = device_time(client, Keyword.get(opts, :at))
    write(client, table, %{"id" => id, "deleted_at" => now, "deleted_by_id" => scope.user.id}, opts)
  end

  @doc """
  Pushes the outbox and applies the response: an accepted row gets its
  `last_synced_at`, a rejected row is replaced by the server's version,
  or dropped when the server has none. Returns the client and the
  response.
  """
  def push(%__MODULE__{} = client) do
    rows = for {table, id} <- client.outbox, do: %{"table" => table, "row" => row(client, table, id)}
    response = wire(Sync.push(client.scope, rows))

    client =
      Enum.reduce(response["accepted"], client, fn %{"table" => table, "id" => id, "last_synced_at" => at},
                                                   client ->
        put_row(client, table, Map.put(row(client, table, id), "last_synced_at", at))
      end)

    client =
      Enum.reduce(response["rejected"], client, fn %{"table" => table, "id" => id, "server" => server},
                                                   client ->
        client = drop_row(client, table, id)
        if server, do: apply_row(client, server), else: client
      end)

    {%{client | outbox: []}, response}
  end

  defp apply_row(client, %{"table" => table, "row" => row}), do: put_row(client, table, row)

  defp put_row(client, table, %{"id" => id} = row) do
    %{client | rows: Map.update(client.rows, table, %{id => row}, &Map.put(&1, id, row))}
  end

  defp drop_row(client, table, id) do
    %{client | rows: Map.update(client.rows, table, %{}, &Map.delete(&1, id))}
  end

  # The device clock: now plus the offset, as the wire format, unless a
  # time is given.
  defp device_time(_client, %DateTime{} = at), do: DateTime.to_iso8601(at)

  defp device_time(%{clock_offset: offset}, nil) do
    DateTime.utc_now() |> DateTime.add(offset, :second) |> DateTime.to_iso8601()
  end

  # As JSON would carry it: string keys, datetimes as text.
  defp wire(term), do: term |> Jason.encode!() |> Jason.decode!()
end
