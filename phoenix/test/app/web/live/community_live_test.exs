defmodule App.Web.CommunityLiveTest do
  use App.Web.ConnCase

  import Phoenix.LiveViewTest
  import App.Data.CommunitiesFixtures
  import App.Data.UsersFixtures

  alias App.Data.Memberships

  setup %{conn: conn} do
    user = user_fixture()
    scope = user_scope_fixture(user)
    %{conn: conn |> log_in_user(user), scope: scope}
  end

  test "shows a community to a member", %{conn: conn, scope: scope} do
    community = joined_community_fixture(scope)

    {:ok, view, _html} = live(conn, ~p"/communities/#{community}")

    assert has_element?(view, "#community-page h1", community.name)
  end

  test "refuses a non-member", %{conn: conn, scope: scope} do
    community = community_fixture()

    assert {:error, {:redirect, %{to: "/communities", flash: flash}}} =
             live(conn, ~p"/communities/#{community}")

    assert flash["error"] =~ "not a member"
  end

  test "refuses a member who left", %{conn: conn, scope: scope} do
    community = joined_community_fixture(scope)
    {:ok, _membership} = Memberships.leave(scope, community.id)

    assert {:error, {:redirect, %{to: "/communities"}}} =
             live(conn, ~p"/communities/#{community}")
  end

  test "shows the community's feed and removes the user's own post from it",
       %{conn: conn, scope: scope} do
    community = joined_community_fixture(scope)
    post = App.Data.PostsFixtures.post_fixture(scope, [community.id], %{body: "In here"})
    elsewhere = App.Data.PostsFixtures.post_fixture(scope, [], %{body: "Not here"})

    {:ok, view, _html} = live(conn, ~p"/communities/#{community}")

    assert has_element?(view, "#posts-#{post.id}", "In here")
    refute has_element?(view, "#posts-#{elsewhere.id}")

    view |> element("#posts-#{post.id} button", "Remove") |> render_click()

    refute has_element?(view, "#posts-#{post.id}")
    assert App.Data.Posting.feed(App.Web.Scope.put_community(scope, community)) == []
  end

  test "leaves from the page", %{conn: conn, scope: scope} do
    community = joined_community_fixture(scope)
    {:ok, view, _html} = live(conn, ~p"/communities/#{community}")

    view |> element("#leave-community") |> render_click()

    assert_redirect(view, ~p"/communities")
    refute Memberships.member?(scope, community.id)
  end
end
