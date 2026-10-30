defmodule App.Web.CommunityLive do
  @moduledoc """
  One community and its feed, for its members. The community comes from
  the path and the membership check from `App.Web.CommunityAuth`.
  """

  use App.Web, :live_view

  alias App.Data.Memberships
  alias App.Data.Posting
  alias App.Data.Posts
  alias App.Data.UsersCommunities

  on_mount {App.Web.CommunityAuth, :require_community_member}

  @impl true
  def mount(_params, _session, socket) do
    scope = socket.assigns.current_scope
    members = UsersCommunities.list(filters: [community_id: scope.community.id])

    {:ok,
     socket
     |> assign(page_title: scope.community.name, member_count: length(members))
     |> stream(:posts, Posting.feed(scope))}
  end

  @impl true
  def render(assigns) do
    ~H"""
    <Layouts.app flash={@flash} current_scope={@current_scope}>
      <div id="community-page" class="space-y-8">
        <.header>
          {@current_scope.community.name}
          <:subtitle>
            A community with {@member_count} member(s)
          </:subtitle>
          <:actions>
            <.button id="leave-community" phx-click="leave" data-confirm="Leave this community?">
              Leave
            </.button>
          </:actions>
        </.header>

        <ul id="posts" phx-update="stream" class="divide-y divide-base-200">
          <li :for={{id, post} <- @streams.posts} id={id} class="space-y-1 py-3">
            <p class="text-sm text-base-content/60">{post.owner.email}</p>
            <p class="whitespace-pre-wrap">{post.body}</p>
            <.button
              :if={post.owner_id == @current_scope.user.id}
              phx-click="remove"
              phx-value-id={post.id}
            >
              Remove from {@current_scope.community.name}
            </.button>
          </li>
        </ul>

        <div class="flex gap-4">
          <.link navigate={~p"/posts"} class="link">Write a post</.link>
          <.link navigate={~p"/communities"} class="link">All communities</.link>
        </div>
      </div>
    </Layouts.app>
    """
  end

  @impl true
  def handle_event("remove", %{"id" => id}, socket) do
    scope = socket.assigns.current_scope

    with %App.Data.Post{} = post <- Posts.get(id),
         {:ok, _row} <- Posting.remove_from_community(scope, post, scope.community.id) do
      {:noreply, stream_delete(socket, :posts, post)}
    else
      _ -> {:noreply, put_flash(socket, :error, "That post is gone from here, or not yours.")}
    end
  end

  def handle_event("leave", _params, socket) do
    scope = socket.assigns.current_scope
    {:ok, _membership} = Memberships.leave(scope, scope.community.id)

    {:noreply,
     socket
     |> put_flash(:info, "You left #{scope.community.name}.")
     |> push_navigate(to: ~p"/communities")}
  end
end
