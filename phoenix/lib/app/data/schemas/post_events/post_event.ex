defmodule App.Data.PostEvent do
  @moduledoc """
  One change to a post. Server-only and append-only, see "Event-sourced
  resources" in `docs/design/offline-sync.md`.
  """

  use Ecto.Schema
  import Ecto.Changeset

  @primary_key {:id, Ecto.UUID, autogenerate: [version: 7]}
  @foreign_key_type Ecto.UUID

  schema "app_post_events" do
    belongs_to :post, App.Data.Post
    belongs_to :user, App.Data.AppUser
    field :type, Ecto.Enum, values: [:create, :update]
    field :base_version, :integer
    field :version, :integer
    field :changes, :map, default: %{}
    field :conflicts, :map, default: %{}

    timestamps(type: :utc_datetime_usec, updated_at: false)
  end

  @fields [:id, :post_id, :user_id, :type, :base_version, :version, :changes, :conflicts]

  def changeset(event, params, _options \\ []) do
    event
    |> cast(params, @fields)
    |> validate_required(@fields -- [:id])
  end
end
