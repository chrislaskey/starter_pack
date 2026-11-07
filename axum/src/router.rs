//! Every route: the JSON API under `/api`, then the Svelte pages. A path the
//! API and the built files don't know gets `index.html`, so the pages'
//! router (`svelte/src/lib/router.js`) shows the page at that path, and
//! every page is reloadable and linkable.

use std::path::Path;

use app_core::database::Database;
use axum::Router;
use axum::routing::get;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

use crate::posts;

pub fn router(svelte_dist: &Path, database: Database) -> Router {
    let pages = ServeDir::new(svelte_dist).fallback(ServeFile::new(svelte_dist.join("index.html")));

    Router::new()
        .route("/api/posts", get(posts::list_posts))
        .route("/api/posts/{id}", get(posts::get_post))
        .with_state(database)
        .fallback_service(pages)
        .layer(TraceLayer::new_for_http())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use tower::ServiceExt;

    use super::*;

    /// A router over the sample posts in an in-memory database and a
    /// stand-in `index.html`.
    async fn test_router() -> Router {
        let dist = std::env::temp_dir().join(format!("app_web_test_{}", std::process::id()));
        fs::create_dir_all(&dist).expect("a temp dir");
        fs::write(dist.join("index.html"), "<html>pages</html>").expect("index.html");
        let database = Database::open_sqlite_in_memory().await.expect("a database");
        app_core::posts::seed_sample_posts(&database)
            .await
            .expect("the sample posts");
        router(&dist, database)
    }

    async fn get(router: &Router, path: &str) -> (StatusCode, Vec<u8>) {
        let request = Request::get(path).body(Body::empty()).expect("a request");
        let response = router.clone().oneshot(request).await.expect("a response");
        let status = response.status();
        let body = response
            .into_body()
            .collect()
            .await
            .expect("a body")
            .to_bytes();
        (status, body.to_vec())
    }

    fn json(body: &[u8]) -> Value {
        serde_json::from_slice(body).expect("json")
    }

    #[tokio::test]
    async fn lists_posts_as_json() {
        let router = test_router().await;
        let (status, body) = get(&router, "/api/posts").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json(&body).as_array().map(Vec::len), Some(3));
    }

    #[tokio::test]
    async fn gets_a_post_as_json() {
        let router = test_router().await;
        let (_, body) = get(&router, "/api/posts").await;
        let listed = json(&body)[0].clone();
        let id = listed["id"].as_str().expect("an id");

        let (status, body) = get(&router, &format!("/api/posts/{id}")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json(&body), listed);
    }

    #[tokio::test]
    async fn answers_a_missing_post_with_404() {
        let router = test_router().await;
        let id = "01900000-0000-7000-8000-000000000000";
        let (status, body) = get(&router, &format!("/api/posts/{id}")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            json(&body),
            json!({ "error": format!("post {id} not found") })
        );
    }

    #[tokio::test]
    async fn rejects_an_id_that_is_not_a_uuid() {
        let router = test_router().await;
        let (status, _) = get(&router, "/api/posts/not-a-uuid").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn answers_a_page_path_with_index_html() {
        let router = test_router().await;
        let (status, body) = get(&router, "/posts/1").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, b"<html>pages</html>");
    }
}
