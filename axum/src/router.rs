//! Every route: the JSON API under `/api`, then the Svelte pages. A path the
//! API and the built files don't know gets `index.html`, so the pages'
//! router (`svelte/src/lib/router.js`) shows the page at that path, and
//! every page is reloadable and linkable.

use std::path::Path;

use app_core::deployment::Deployment;
use axum::Router;
use axum::routing::{get, post};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

use crate::{posts, sync};

pub fn router(svelte_dist: &Path, deployment: Deployment) -> Router {
    let pages = ServeDir::new(svelte_dist).fallback(ServeFile::new(svelte_dist.join("index.html")));

    Router::new()
        .route("/api/posts", get(posts::list).post(posts::create))
        .route(
            "/api/posts/{id}",
            get(posts::get).put(posts::update).delete(posts::delete),
        )
        .route("/api/sync/push", post(sync::push))
        .route("/api/sync/pull", post(sync::pull))
        .with_state(deployment)
        .fallback_service(pages)
        .layer(TraceLayer::new_for_http())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode, header};
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use tower::ServiceExt;

    use super::*;

    const MISSING_ID: &str = "01900000-0000-7000-8000-000000000000";

    /// A router over an in-memory database with one post, and a stand-in
    /// `index.html`.
    async fn test_router() -> (Router, Value) {
        let dist = std::env::temp_dir().join(format!("app_web_test_{}", std::process::id()));
        fs::create_dir_all(&dist).expect("a temp dir");
        fs::write(dist.join("index.html"), "<html>pages</html>").expect("index.html");
        let database = app_core::database::Database::open_sqlite_in_memory()
            .await
            .expect("a database");
        let deployment = Deployment::new(database, app_core::deployment::Role::Server);
        let post = app_core::posts::create(&deployment, "the first post")
            .await
            .expect("a post");
        let post = serde_json::to_value(post).expect("json");
        (router(&dist, deployment), post)
    }

    async fn send(
        router: &Router,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let request = Request::builder().method(method).uri(path);
        let request = match body {
            Some(body) => request
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string())),
            None => request.body(Body::empty()),
        }
        .expect("a request");
        let response = router.clone().oneshot(request).await.expect("a response");
        let status = response.status();
        let body = response
            .into_body()
            .collect()
            .await
            .expect("a body")
            .to_bytes();
        let body = if body.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&body)
                .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&body).into()))
        };
        (status, body)
    }

    async fn get(router: &Router, path: &str) -> (StatusCode, Value) {
        send(router, Method::GET, path, None).await
    }

    #[tokio::test]
    async fn posts_list_as_json() {
        let (router, post) = test_router().await;
        let (status, body) = get(&router, "/api/posts").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, json!([post]));
    }

    #[tokio::test]
    async fn posts_get_as_json() {
        let (router, post) = test_router().await;
        let (status, body) = get(
            &router,
            &format!("/api/posts/{}", post["id"].as_str().unwrap()),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, post);
    }

    #[tokio::test]
    async fn posts_get_missing_is_404() {
        let (router, _) = test_router().await;
        let (status, body) = get(&router, &format!("/api/posts/{MISSING_ID}")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            body,
            json!({ "error": format!("post {MISSING_ID} not found") })
        );
    }

    #[tokio::test]
    async fn posts_get_rejects_an_id_that_is_not_a_uuid() {
        let (router, _) = test_router().await;
        let (status, _) = get(&router, "/api/posts/not-a-uuid").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn posts_create() {
        let (router, _) = test_router().await;
        let (status, created) = send(
            &router,
            Method::POST,
            "/api/posts",
            Some(json!({ "body": "a new post" })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(created["body"], json!("a new post"));

        let (_, posts) = get(&router, "/api/posts").await;
        assert_eq!(posts[0], created);
    }

    #[tokio::test]
    async fn posts_create_refuses_blank() {
        let (router, _) = test_router().await;
        let (status, body) = send(
            &router,
            Method::POST,
            "/api/posts",
            Some(json!({ "body": "   " })),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body, json!({ "error": "a post needs some text" }));
    }

    #[tokio::test]
    async fn posts_update() {
        let (router, post) = test_router().await;
        let path = format!("/api/posts/{}", post["id"].as_str().unwrap());
        let (status, updated) = send(
            &router,
            Method::PUT,
            &path,
            Some(json!({ "body": "edited" })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(updated["body"], json!("edited"));
        assert_eq!(updated["inserted_at"], post["inserted_at"]);

        let (_, read) = get(&router, &path).await;
        assert_eq!(read, updated);
    }

    #[tokio::test]
    async fn posts_delete() {
        let (router, post) = test_router().await;
        let path = format!("/api/posts/{}", post["id"].as_str().unwrap());
        let (status, _) = send(&router, Method::DELETE, &path, None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);

        let (status, _) = get(&router, &path).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (status, _) = send(&router, Method::DELETE, &path, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn sync_push_applies_an_event_and_pull_returns_the_row() {
        let (router, post) = test_router().await;
        let event = json!({
            "id": "01900000-0000-7000-8000-000000000001",
            "table": "app_posts",
            "row_id": post["id"],
            "type": "update",
            "base_version": 1,
            "base": { "body": "the first post" },
            "changes": { "body": "pushed" },
            "made_at": "2026-11-08T10:00:00Z"
        });
        let (status, body) = send(
            &router,
            Method::POST,
            "/api/sync/push",
            Some(json!({ "events": [event] })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["events"][0]["status"], json!("applied"));
        assert_eq!(body["events"][0]["version"], json!(2));
        assert_eq!(body["events"][0]["server"]["row"]["body"], json!("pushed"));

        let (status, body) = send(&router, Method::POST, "/api/sync/pull", Some(json!({}))).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["rows"][0]["table"], json!("app_posts"));
        assert_eq!(body["rows"][0]["row"]["body"], json!("pushed"));
        assert_eq!(body["next"], Value::Null);
    }

    #[tokio::test]
    async fn sync_push_rejects_a_malformed_request() {
        let (router, _) = test_router().await;
        let (status, _) = send(
            &router,
            Method::POST,
            "/api/sync/push",
            Some(json!({ "events": [{ "id": "x" }] })),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn pages_answer_any_path_with_index_html() {
        let (router, _) = test_router().await;
        let (status, body) = get(&router, "/posts/new").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, json!("<html>pages</html>"));
    }
}
