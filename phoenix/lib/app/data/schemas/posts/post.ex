defmodule App.Data.Post do
  @moduledoc """
  A post: the first resource, written by one author and in the
  communities the author chose, see `App.Data.PostCommunity`. The
  reference for every rule a resource follows, see phase 3 of
  `docs/design/build-order.md`.

  `last_synced_at` is set on every write, as the sync protocol in
  `docs/design/offline-sync.md` requires of every synced table.

  `changeset/3` is marked `@universal_changeset true`: `mix app.rust` writes
  it as `post.rs` for the desktop and mobile apps, and this app runs that
  Rust too, see `priv/rust/README.md`.
  """

  use Ecto.Schema
  use App.Data.Synced
  use App.Data.UniversalChangeset
  import Ecto.Changeset

  @primary_key {:id, Ecto.UUID, autogenerate: [version: 7]}
  @foreign_key_type Ecto.UUID

  schema "app_posts" do
    belongs_to :author, App.Data.AppUser
    field :body, :string
    field :last_synced_at, :utc_datetime_usec
    field :deleted_at, :utc_datetime_usec
    field :deleted_by_id, Ecto.UUID

    timestamps(type: :utc_datetime_usec)
  end

  @doc """
  Writing or editing a post. The author is the `:author_id` option, set
  by the flow, never a param.
  """
  @universal_changeset true
  def changeset(post, params, options \\ []) do
    post
    |> cast(params, [:body])
    |> put_author(Keyword.get(options, :author_id))
    |> validate_required([:author_id, :body])
    |> validate_length(:body, max: 10_000)
    |> put_change(:last_synced_at, DateTime.utc_now())
  end

  defp put_author(changeset, nil), do: changeset
  defp put_author(changeset, author_id), do: put_change(changeset, :author_id, author_id)

  @doc """
  Deleting a post: a soft delete, with no cascade of its own since reads
  of its community rows join through it.
  """
  def delete_changeset(post, params, _options \\ []) do
    post
    |> cast(params, [:deleted_at, :deleted_by_id])
    |> validate_required([:deleted_at, :deleted_by_id])
    |> put_change(:last_synced_at, DateTime.utc_now())
  end
end
