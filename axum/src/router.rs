//! Every route: the JSON API under `/api`, then the Svelte pages. A path the
//! API and the built files don't know gets `index.html`, so the pages'
//! router (`svelte/src/lib/router.js`) shows the page at that path, and
//! every page is reloadable and linkable.

use std::path::Path;

use axum::Router;
use axum::routing::get;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

use crate::posts;

pub fn router(svelte_dist: &Path) -> Router {
    let pages = ServeDir::new(svelte_dist).fallback(ServeFile::new(svelte_dist.join("index.html")));

    Router::new()
        .route("/api/posts", get(posts::list_posts))
        .route("/api/posts/{id}", get(posts::get_post))
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

    async fn send(request: Request<Body>) -> (StatusCode, Vec<u8>) {
        let dist = std::env::temp_dir().join(format!("app_web_test_{}", std::process::id()));
        fs::create_dir_all(&dist).expect("a temp dir");
        fs::write(dist.join("index.html"), "<html>pages</html>").expect("index.html");

        let response = router(&dist).oneshot(request).await.expect("a response");
        let status = response.status();
        let body = response
            .into_body()
            .collect()
            .await
            .expect("a body")
            .to_bytes();
        (status, body.to_vec())
    }

    fn get_request(path: &str) -> Request<Body> {
        Request::get(path).body(Body::empty()).expect("a request")
    }

    #[tokio::test]
    async fn lists_posts_as_json() {
        let (status, body) = send(get_request("/api/posts")).await;
        assert_eq!(status, StatusCode::OK);
        let posts: Value = serde_json::from_slice(&body).expect("json");
        assert_eq!(posts.as_array().map(Vec::len), Some(3));
    }

    #[tokio::test]
    async fn gets_a_post_as_json() {
        let (status, body) = send(get_request("/api/posts/1")).await;
        assert_eq!(status, StatusCode::OK);
        let post: Value = serde_json::from_slice(&body).expect("json");
        assert_eq!(post["id"], json!("1"));
    }

    #[tokio::test]
    async fn answers_a_missing_post_with_404() {
        let (status, body) = send(get_request("/api/posts/missing")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let error: Value = serde_json::from_slice(&body).expect("json");
        assert_eq!(error, json!({ "error": "post missing not found" }));
    }

    #[tokio::test]
    async fn answers_a_page_path_with_index_html() {
        let (status, body) = send(get_request("/posts/1")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, b"<html>pages</html>");
    }
}
