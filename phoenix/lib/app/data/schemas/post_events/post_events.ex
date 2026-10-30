defmodule App.Data.PostEvents do
  @moduledoc """
  The data context over `App.Data.PostEvent`, oldest first.
  """

  use App.Data.Context, schema: App.Data.PostEvent, sorts: [:inserted_at, :id]
end
