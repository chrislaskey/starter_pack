defmodule App.Web.ExplorationsLiveTest do
  use App.Web.ConnCase

  import Phoenix.LiveViewTest

  alias App.Web.ExplorationsLive

  test "lists every exploration", %{conn: conn} do
    {:ok, view, _html} = live(conn, ~p"/explorations")

    for exploration <- ExplorationsLive.explorations() do
      assert has_element?(view, "a[href='/explorations/#{exploration.slug}']", exploration.title)
    end
  end

  test "shows each exploration with its stage and controls", %{conn: conn} do
    for exploration <- ExplorationsLive.explorations() do
      {:ok, view, _html} = live(conn, ~p"/explorations/#{exploration.slug}")

      assert has_element?(view, "#stage-#{exploration.slug}[data-exploration='#{exploration.slug}']")
      assert has_element?(view, "#exploration-controls")
    end
  end

  test "sends an unknown slug back to the list", %{conn: conn} do
    assert {:error, {:live_redirect, %{to: "/explorations"}}} = live(conn, ~p"/explorations/nope")
  end

  test "a fired event reaches every tab, marked as mine only in the tab that fired it", %{
    conn: conn
  } do
    {:ok, mine, _html} = live(conn, ~p"/explorations/waves")
    {:ok, theirs, _html} = live(conn, ~p"/explorations/ripples")

    mine |> element("#exploration-controls button", "Post") |> render_click()

    assert_push_event(mine, "viz", %{kind: "post", mine: true})
    assert_push_event(theirs, "viz", %{kind: "post", mine: false})
  end

  test "opening a second tab is a join in the first", %{conn: conn} do
    {:ok, first, _html} = live(conn, ~p"/explorations/waves")
    {:ok, second, _html} = live(conn, ~p"/explorations/waves")

    assert_push_event(first, "viz", %{kind: "join", mine: false})
    assert render(first) =~ "2 tabs here"
    assert render(second) =~ "2 tabs here"
  end
end
