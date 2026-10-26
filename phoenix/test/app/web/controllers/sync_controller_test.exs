defmodule App.Web.SyncControllerTest do
  use App.Web.ConnCase

  import App.Data.CommunitiesFixtures
  import App.Data.PostsFixtures
  import App.Data.UsersFixtures

  alias App.Data.Accounts
  alias App.Data.Posts

  setup %{conn: conn} do
    user = user_fixture()
    scope = user_scope_fixture(user)
    token = Accounts.generate_user_api_token(user)

    conn =
      conn
      |> put_req_header("authorization", "Bearer " <> token)
      |> put_req_header("content-type", "application/json")

    %{conn: conn, scope: scope, user: user}
  end

  defp post_json(conn, path, body), do: post(conn, path, Jason.encode!(body))

  test "refuses a request without a valid token", %{conn: conn} do
    conn = conn |> delete_req_header("authorization") |> post_json(~p"/sync/pull", %{})
    assert json_response(conn, 401)
  end

  test "pulls, pushes and backfills", %{conn: conn, scope: scope} do
    community = joined_community_fixture(scope)
    existing = post_fixture(scope, [community.id])

    pulled = conn |> post_json(~p"/sync/pull", %{"limit" => 10}) |> json_response(200)
    assert %{"rows" => rows, "next" => nil, "cursor" => %{"last_synced_at" => _, "id" => _}} = pulled
    assert existing.id in for(%{"table" => "app_posts", "row" => row} <- rows, do: row["id"])

    now = DateTime.to_iso8601(DateTime.utc_now())
    id = Ecto.UUID.generate(version: 7)

    pushed =
      conn
      |> post_json(~p"/sync/push", %{
        "rows" => [
          %{
            "table" => "app_posts",
            "row" => %{"id" => id, "body" => "over http", "author_id" => scope.user.id, "updated_at" => now}
          }
        ]
      })
      |> json_response(200)

    assert %{"accepted" => [%{"table" => "app_posts", "id" => ^id}], "rejected" => []} = pushed
    assert Posts.get(id).body == "over http"

    backfilled = conn |> post_json(~p"/sync/backfill/#{community.id}", %{}) |> json_response(200)

    assert [
             %{"table" => "app_users"},
             %{"table" => "app_posts", "row" => %{"id" => existing_id}},
             %{"table" => "app_posts_communities"}
           ] = backfilled["rows"]

    assert existing_id == existing.id
  end

  test "rejects a cursor that is not a datetime", %{conn: conn} do
    body = %{"cursor" => %{"last_synced_at" => "yesterday", "id" => "x"}}
    assert conn |> post_json(~p"/sync/pull", body) |> json_response(400)
  end

  test "rejects a push without a list of rows", %{conn: conn} do
    assert conn |> post_json(~p"/sync/push", %{"rows" => "nope"}) |> json_response(400)
  end

  test "refuses a backfill of a community the user is not a member of", %{conn: conn} do
    other = user_scope_fixture(user_fixture())
    community = joined_community_fixture(other)
    assert conn |> post_json(~p"/sync/backfill/#{community.id}", %{}) |> json_response(403)
  end
end
