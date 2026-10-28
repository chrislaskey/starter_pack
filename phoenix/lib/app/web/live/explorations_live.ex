defmodule App.Web.ExplorationsLive do
  @moduledoc """
  Scratch pages for ambient animation: ways a page could show what's
  happening in the app, like people arriving and leaving or posts being
  written, in the background without getting in the way.

  `/explorations` lists the ideas and `/explorations/:slug` shows one over
  stand-in page content, with a panel that fires events. Every tab open on
  these pages is in one room, tracked by `App.Web.Presence`: opening a
  second tab is a real join, and the panel's buttons reach every tab. The
  animations are the modules in `assets/js/explorations/`, one per slug,
  run by the `Exploration` hook in `assets/js/explorations/hook.js`.

  Nothing here is wired to the real app; the events are made up on the
  spot. Picking an idea is a separate job from building it.
  """

  use App.Web, :live_view

  alias App.Web.Presence

  @topic "explorations"

  @kinds ~w(join leave post react celebrate)

  @explorations [
    %{
      slug: "waves",
      title: "Waves",
      tech: "SVG + D3",
      note: "Three waves along the bottom, one per color. An event drops a ripple into them."
    },
    %{
      slug: "waves-slow",
      title: "Waves, slow at rest",
      tech: "SVG + D3",
      note: "The waves barely move until an event speeds them up, then ease back to a crawl."
    },
    %{
      slug: "waves-still",
      title: "Waves, still at rest",
      tech: "SVG + D3",
      note: "The waves don't move at all until an event, then damp back to stillness."
    },
    %{
      slug: "waves-tide",
      title: "Waves, tide",
      tech: "SVG + D3",
      note: "Height and speed follow the room's activity: low and slow when quiet, tall when busy."
    },
    %{
      slug: "waves-lines",
      title: "Waves, lines",
      tech: "SVG + D3",
      note: "Thin overlapping wave lines instead of fills. A ripple thickens the line as it passes."
    },
    %{
      slug: "waves-lines-still",
      title: "Waves, lines still at rest",
      tech: "SVG + D3",
      note: "The thin lines sit still until an event sets them moving and thickens them."
    },
    %{
      slug: "ribbon",
      title: "Ribbons",
      tech: "SVG + D3",
      note: "Thin flowing lines across the page. An event sends a swell along the matching line."
    },
    %{
      slug: "ribbon-braid",
      title: "Ribbons, braided",
      tech: "SVG + D3",
      note: "The three lines share one band and constantly cross, with overlaps deepening in color."
    },
    %{
      slug: "ribbon-thick",
      title: "Ribbons, thickness",
      tech: "SVG + D3",
      note: "Hairlines at rest. An event sends a bulge of width along the matching line."
    },
    %{
      slug: "ribbon-still",
      title: "Ribbons, still at rest",
      tech: "SVG + D3",
      note: "Frozen until an event sets the line flowing from where it landed, damping back to still."
    },
    %{
      slug: "ribbon-slow",
      title: "Ribbons, slow at rest",
      tech: "SVG + D3",
      note: "A tenth of the flow at rest. An event speeds it up for a few seconds."
    },
    %{
      slug: "ribbon-silk",
      title: "Ribbons, silk",
      tech: "SVG + D3",
      note: "Wide translucent bands layered like silk. An event brightens and widens the matching one."
    },
    %{
      slug: "goo",
      title: "Goo",
      tech: "SVG filter",
      note: "A gooey blob per person, clustered in a corner. Joining merges in, leaving pulls away."
    },
    %{
      slug: "heartbeat",
      title: "Heartbeat",
      tech: "Canvas",
      note: "An ECG trace scrolling along the bottom. Each event is a beat in its color."
    },
    %{
      slug: "seismograph",
      title: "Seismograph",
      tech: "SVG + D3",
      note: "An activity chart scrolling along the bottom: how busy the room is right now."
    },
    %{
      slug: "ripples",
      title: "Ripples",
      tech: "Canvas",
      note: "Rings spreading out from a point, like a drop on still water."
    },
    %{
      slug: "fireflies",
      title: "Fireflies",
      tech: "Canvas",
      note: "A glowing dot per person drifting across the page. Posting bursts a ring."
    },
    %{
      slug: "constellation",
      title: "Constellation",
      tech: "Canvas",
      note: "Dots per person joined by faint lines. A post sends light along the lines."
    },
    %{
      slug: "orbit",
      title: "Orbit",
      tech: "Canvas",
      note: "You at the corner, everyone else a satellite. Joining spirals in."
    },
    %{
      slug: "sonar",
      title: "Sonar",
      tech: "Canvas widget",
      note: "A small radar in the corner with a dot per person. The quietest idea here."
    },
    %{
      slug: "dotgrid",
      title: "Dot grid",
      tech: "Canvas",
      note: "Dotted paper behind the page. An event sends a color wave through the dots."
    },
    %{
      slug: "confetti",
      title: "Confetti",
      tech: "Canvas",
      note: "Nothing until an event, then a burst. The big-reaction idea."
    },
    %{
      slug: "bubbles",
      title: "Bubbles",
      tech: "Canvas",
      note: "Bubbles rise from the bottom edge and pop, one per event."
    },
    %{
      slug: "comet",
      title: "Comet bar",
      tech: "Canvas",
      note: "A thin bar along the top. Events send a glowing comet across it."
    },
    %{
      slug: "meteors",
      title: "Meteors",
      tech: "Canvas",
      note: "Shooting stars crossing the page, one per event."
    },
    %{
      slug: "edgeglow",
      title: "Edge glow",
      tech: "CSS",
      note: "The viewport edges breathe color: your actions at the bottom, others' at the top."
    },
    %{
      slug: "aurora",
      title: "Aurora",
      tech: "CSS",
      note: "Blurred color blobs drifting behind the page. An event makes one swell."
    },
    %{
      slug: "terrain",
      title: "Terrain",
      tech: "Three.js",
      note: "A field of points rolling on noise along the bottom. Events ripple through it."
    },
    %{
      slug: "nebula",
      title: "Nebula",
      tech: "Three.js shader",
      note: "Slow fog in the palette colors. Events brighten it where they land."
    }
  ]

  def explorations, do: @explorations

  @impl true
  def mount(_params, _session, socket) do
    if connected?(socket) do
      Phoenix.PubSub.subscribe(App.PubSub, @topic)
      {:ok, _} = Presence.track(self(), @topic, socket.id, %{})
    end

    {:ok, assign(socket, explorations: @explorations, present: present())}
  end

  @impl true
  def handle_params(%{"slug" => slug}, _uri, socket) do
    case Enum.find_index(@explorations, &(&1.slug == slug)) do
      nil ->
        {:noreply,
         socket
         |> put_flash(:error, "There is no exploration called #{slug}")
         |> push_navigate(to: ~p"/explorations")}

      index ->
        {:noreply,
         assign(socket,
           page_title: Enum.at(@explorations, index).title,
           exploration: Enum.at(@explorations, index),
           previous: Enum.at(@explorations, index - 1),
           next: Enum.at(@explorations, index + 1)
         )}
    end
  end

  def handle_params(_params, _uri, socket) do
    {:noreply, assign(socket, page_title: "Explorations", exploration: nil)}
  end

  @impl true
  def handle_event("fire", %{"kind" => kind}, socket) when kind in @kinds do
    event =
      case kind do
        "join" -> %{kind: kind, id: "ghost-#{System.unique_integer([:positive])}", from: nil}
        "leave" -> %{kind: kind, id: nil, from: nil}
        _ -> %{kind: kind, id: socket.id, from: socket.id}
      end

    Phoenix.PubSub.broadcast(App.PubSub, @topic, {:viz, event})
    {:noreply, socket}
  end

  @impl true
  def handle_info({:viz, event}, socket) do
    {:noreply, push(socket, event.kind, event.id, event.from == socket.id)}
  end

  def handle_info(%Phoenix.Socket.Broadcast{event: "presence_diff", payload: diff}, socket) do
    others = fn presences -> Map.keys(presences) -- [socket.id] end

    socket =
      Enum.reduce(others.(diff.joins), socket, &push(&2, "join", &1, false))
      |> then(&Enum.reduce(others.(diff.leaves), &1, fn id, s -> push(s, "leave", id, false) end))

    {:noreply, assign(socket, present: present())}
  end

  defp push(socket, kind, id, mine) do
    push_event(socket, "viz", %{kind: kind, id: id, mine: mine})
  end

  defp present, do: map_size(Presence.list(@topic))

  @impl true
  def render(%{exploration: nil} = assigns) do
    ~H"""
    <Layouts.app flash={@flash} current_scope={@current_scope}>
      <div id="explorations" class="space-y-8">
        <.header>
          Explorations
          <:subtitle>
            Ambient animation: ways the page could show what's happening without
            getting in the way. Open one in two tabs to see other people's actions.
          </:subtitle>
        </.header>

        <ul class="grid gap-3 sm:grid-cols-2">
          <li :for={exploration <- @explorations}>
            <.link
              patch={~p"/explorations/#{exploration.slug}"}
              class="block rounded-box border border-base-300 p-4 transition hover:border-primary hover:bg-base-200"
            >
              <div class="flex items-baseline justify-between gap-2">
                <span class="font-semibold">{exploration.title}</span>
                <span class="text-xs text-base-content/60">{exploration.tech}</span>
              </div>
              <p class="mt-1 text-sm text-base-content/70">{exploration.note}</p>
            </.link>
          </li>
        </ul>
      </div>
    </Layouts.app>
    """
  end

  def render(assigns) do
    ~H"""
    <div
      id={"stage-#{@exploration.slug}"}
      phx-hook="Exploration"
      phx-update="ignore"
      data-exploration={@exploration.slug}
      class="pointer-events-none fixed inset-0 -z-10 overflow-hidden"
    >
    </div>

    <Layouts.app flash={@flash} current_scope={@current_scope}>
      <div id={"exploration-#{@exploration.slug}"} class="space-y-8">
        <.header>
          {@exploration.title}
          <:subtitle>{@exploration.note}</:subtitle>
          <:actions>
            <div class="flex items-center gap-3 text-sm">
              <.link :if={@previous} patch={~p"/explorations/#{@previous.slug}"} class="link">
                &larr; {@previous.title}
              </.link>
              <.link patch={~p"/explorations"} class="link">All</.link>
              <.link :if={@next} patch={~p"/explorations/#{@next.slug}"} class="link">
                {@next.title} &rarr;
              </.link>
            </div>
          </:actions>
        </.header>

        <.stand_in_feed />
      </div>
    </Layouts.app>

    <aside
      id="exploration-controls"
      phx-hook="ExplorationControls"
      class="fixed bottom-4 right-4 z-20 w-56 space-y-3 rounded-box border border-base-300 bg-base-100/90 p-4 shadow-lg backdrop-blur"
    >
      <p class="text-xs text-base-content/60">
        {@present} {if @present == 1, do: "tab", else: "tabs"} here · {@exploration.tech}
      </p>
      <div class="grid grid-cols-2 gap-2">
        <.button
          :for={kind <- ["join", "leave", "post", "react"]}
          phx-click="fire"
          phx-value-kind={kind}
          class="btn btn-sm btn-soft btn-primary"
        >
          {String.capitalize(kind)}
        </.button>
        <.button phx-click="fire" phx-value-kind="celebrate" class="btn btn-sm btn-primary col-span-2">
          Celebrate
        </.button>
      </div>
      <.input
        type="checkbox"
        id="simulate"
        name="simulate"
        value={false}
        label="Simulate other people"
      />
    </aside>
    """
  end

  # Some page to see the animation behind. The names and words mean nothing.
  defp stand_in_feed(assigns) do
    assigns =
      assign(assigns, :posts, [
        {"mira", "Morning everyone. The garden beds are in, if anyone wants to come by this weekend."},
        {"theo", "Count me in for Saturday. I'll bring the second wheelbarrow."},
        {"june", "Photos from last week are up. The tomatoes did better than we thought."},
        {"ravi", "Has anyone tried the new bakery on the corner? The rye is excellent."},
        {"mira", "Reminder that the shed key moved to the hook by the gate."}
      ])

    ~H"""
    <div class="space-y-4">
      <div class="space-y-2">
        <textarea class="textarea w-full" rows="3" placeholder="Write a post" disabled></textarea>
        <.button class="btn btn-primary btn-sm" disabled>Post</.button>
      </div>
      <ul class="divide-y divide-base-200">
        <li :for={{name, body} <- @posts} class="space-y-1 py-3">
          <p class="text-sm text-base-content/60">{name}</p>
          <p>{body}</p>
        </li>
      </ul>
    </div>
    """
  end
end
