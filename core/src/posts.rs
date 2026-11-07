//! Posts: the first resource. Hard-coded for now; the database comes later.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A post as the pages show it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Post {
    pub id: String,
    pub body: String,
    /// When the post was written, as an RFC 3339 timestamp.
    pub inserted_at: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PostError {
    #[error("post {id} not found")]
    NotFound { id: String },
}

/// Every post, newest first.
pub fn list_posts() -> Vec<Post> {
    sample_posts()
}

/// One post by its id.
pub fn get_post(id: &str) -> Result<Post, PostError> {
    sample_posts()
        .into_iter()
        .find(|post| post.id == id)
        .ok_or_else(|| PostError::NotFound { id: id.to_string() })
}

fn sample_posts() -> Vec<Post> {
    vec![
        Post {
            id: "3".to_string(),
            body: "The same Svelte pages run in the browser, on the desktop and on a phone."
                .to_string(),
            inserted_at: "2026-11-07T09:30:00Z".to_string(),
        },
        Post {
            id: "2".to_string(),
            body: "The web server answers over HTTP, the apps over Tauri commands, and both call app_core."
                .to_string(),
            inserted_at: "2026-11-06T17:15:00Z".to_string(),
        },
        Post {
            id: "1".to_string(),
            body: "Hello from app_core.".to_string(),
            inserted_at: "2026-11-05T08:00:00Z".to_string(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_posts_newest_first() {
        let posts = list_posts();
        assert_eq!(posts.len(), 3);
        assert!(
            posts
                .windows(2)
                .all(|pair| pair[0].inserted_at > pair[1].inserted_at)
        );
    }

    #[test]
    fn gets_a_post_by_id() {
        let post = get_post("1").expect("post 1 exists");
        assert_eq!(post.body, "Hello from app_core.");
    }

    #[test]
    fn reports_a_missing_post() {
        assert_eq!(
            get_post("missing"),
            Err(PostError::NotFound {
                id: "missing".to_string()
            })
        );
    }
}
