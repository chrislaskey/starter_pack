defmodule App.Web.PostsLive do
  @moduledoc """
  Everything the user can read across their communities, newest first,
  and a form to write a post into the communities they choose.
  """

  use App.Web, :live_view

  alias App.Data.Post
  alias App.Data.Posting
  alias App.Data.Posts
  alias App.Data.UsersCommunities

  @impl true
  def mount(_params, _session, socket) do
    scope = socket.assigns.current_scope

    communities =
      [filters: [user_id: scope.user.id], preloads: [:community]]
      |> UsersCommunities.list()
      |> Enum.map(& &1.community)
      |> Enum.sort_by(& &1.name)

    {:ok,
     socket
     |> assign(page_title: "Posts", form: new_form(), communities: communities)
     |> stream(:posts, Posting.feed(scope))}
  end

  @impl true
  def render(assigns) do
    ~H"""
    <Layouts.app flash={@flash} current_scope={@current_scope}>
      <div id="posts-page" class="space-y-8">
        <.header>
          Posts
          <:subtitle>Everything you can read in your communities</:subtitle>
          <:actions>
            <.link navigate={~p"/communities"} class="link">Communities</.link>
          </:actions>
        </.header>

        <.form for={@form} id="post-form" phx-submit="write" class="space-y-2">
          <.input field={@form[:body]} type="textarea" label="Write a post" />
          <.input
            field={@form[:community_ids]}
            type="select"
            multiple
            label="In these communities"
            options={for community <- @communities, do: {community.name, community.id}}
          />
          <.button variant="primary" phx-disable-with="Posting...">Post</.button>
        </.form>

        <ul id="posts" phx-update="stream" class="divide-y divide-base-200">
          <li :for={{id, post} <- @streams.posts} id={id} class="space-y-1 py-3">
            <p class="text-sm text-base-content/60">{post.owner.email}</p>
            <p class="whitespace-pre-wrap">{post.body}</p>
            <.button
              :if={post.owner_id == @current_scope.user.id}
              phx-click="delete"
              phx-value-id={post.id}
              data-confirm="Delete this post?"
            >
              Delete
            </.button>
          </li>
        </ul>
      </div>
    </Layouts.app>
    """
  end

  @impl true
  def handle_event("write", %{"post" => params}, socket) do
    scope = socket.assigns.current_scope
    community_ids = Map.get(params, "community_ids", [])

    case Posting.write(scope, params, community_ids) do
      {:ok, post} ->
        {:noreply,
         socket
         |> assign(form: new_form())
         |> stream_insert(:posts, Posts.get(post.id, preloads: [:owner]), at: 0)}

      {:error, %Ecto.Changeset{} = changeset} ->
        {:noreply, assign(socket, form: to_form(changeset))}

      {:error, :not_a_member} ->
        {:noreply, put_flash(socket, :error, "You can only post in communities you are a member of.")}
    end
  end

  def handle_event("delete", %{"id" => id}, socket) do
    scope = socket.assigns.current_scope

    with %Post{} = post <- Posts.get(id),
         {:ok, _post} <- Posting.delete(scope, post) do
      {:noreply, stream_delete(socket, :posts, post)}
    else
      _ -> {:noreply, put_flash(socket, :error, "That post is gone, or not yours.")}
    end
  end

  defp new_form, do: to_form(Posts.change(%Post{}))
end
