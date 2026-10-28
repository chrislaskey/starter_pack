defmodule App.Web.Presence do
  @moduledoc """
  Who is on a page right now. `App.Web.ExplorationsLive` tracks each open
  tab on the `"explorations"` topic, so the animations there see real joins
  and leaves.
  """

  use Phoenix.Presence, otp_app: :app, pubsub_server: App.PubSub
end
