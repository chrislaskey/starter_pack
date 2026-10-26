defmodule App.Web.PostsLiveTest do
  use App.Web.ConnCase

  import Phoenix.LiveViewTest
  import App.Data.CommunitiesFixtures
  import App.Data.PostsFixtures
  import App.Data.UsersFixtures

  alias App.Data.Posting
  alias App.Web.Scope

  setup %{conn: conn} do
    user = user_fixture()
    scope = user_scope_fixture(user)
    %{conn: conn |> log_in_user(user), scope: scope}
  end

  test "writes a post into the ticked communities", %{conn: conn, scope: scope} do
    community = joined_community_fixture(scope)
    {:ok, view, _html} = live(conn, ~p"/posts")

    view
    |> form("#post-form", post: %{body: "Hello", community_ids: [community.id]})
    |> render_submit()

    assert has_element?(view, "#posts li", "Hello")
    assert [post] = Posting.feed(Scope.put_community(scope, community))
    assert post.body == "Hello"
  end

  test "requires a body", %{conn: conn} do
    {:ok, view, _html} = live(conn, ~p"/posts")

    html =
      view
      |> form("#post-form", post: %{body: ""})
      |> render_submit()

    assert html =~ "can&#39;t be blank"
  end

  test "lists what the user can read and deletes their own", %{conn: conn, scope: scope} do
    community = joined_community_fixture(scope)
    post = post_fixture(scope, [community.id], %{body: "Mine"})

    other = user_scope_fixture(user_fixture())
    hidden = post_fixture(other, [], %{body: "Theirs alone"})

    {:ok, view, _html} = live(conn, ~p"/posts")

    assert has_element?(view, "#posts-#{post.id}", "Mine")
    refute has_element?(view, "#posts-#{hidden.id}")

    view |> element("#posts-#{post.id} button", "Delete") |> render_click()

    refute has_element?(view, "#posts-#{post.id}")
    assert Posting.feed(scope) == []
  end
end
