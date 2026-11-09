//! Every route: the JSON API under `/api`, then the Svelte pages. A path the
//! API and the built files don't know gets `index.html`, so the pages'
//! router (`svelte/src/lib/router.js`) shows the page at that path, and
//! every page is reloadable and linkable.

use std::path::Path;

use app_core::deployment::Deployment;
use app_core::mailer::Mailer;
use app_core::storage::Storage;
use axum::Router;
use axum::extract::FromRef;
use axum::routing::{delete, get, post, put};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

use crate::rate_limits::{self, RateLimits};
use crate::{accounts, avatars, communities, memberships, posts, sync, users};

/// What every handler may take from the server: the database, where its
/// email goes, the bucket uploads go to if any, and the rate limits; and
/// whether the client IP is the proxy's header, see `rate_limits`.
#[derive(Clone)]
pub struct AppState {
    pub deployment: Deployment,
    pub mailer: Mailer,
    pub storage: Option<Storage>,
    pub rate_limits: RateLimits,
    pub trust_proxy_headers: bool,
}

impl FromRef<AppState> for Deployment {
    fn from_ref(state: &AppState) -> Deployment {
        state.deployment.clone()
    }
}

impl FromRef<AppState> for Mailer {
    fn from_ref(state: &AppState) -> Mailer {
        state.mailer.clone()
    }
}

impl FromRef<AppState> for Option<Storage> {
    fn from_ref(state: &AppState) -> Option<Storage> {
        state.storage.clone()
    }
}

impl FromRef<AppState> for RateLimits {
    fn from_ref(state: &AppState) -> RateLimits {
        state.rate_limits.clone()
    }
}

pub fn router(svelte_dist: &Path, state: AppState) -> Router {
    let pages = ServeDir::new(svelte_dist).fallback(ServeFile::new(svelte_dist.join("index.html")));
    let accounts = Router::new()
        .route("/api/accounts/sign-up", post(accounts::sign_up))
        .route("/api/accounts/sign-in", post(accounts::sign_in))
        .route("/api/accounts/code", post(accounts::code))
        .route("/api/accounts/code/resend", post(accounts::code_resend))
        .route("/api/accounts/device-token", post(accounts::device_token))
        .route("/api/accounts/sign-out", delete(accounts::sign_out))
        .layer(rate_limits::per_ip_layer(state.trust_proxy_headers));

    Router::new()
        .merge(accounts)
        .route("/api/users/me", get(users::me))
        .route("/api/avatars", put(avatars::update))
        .route("/api/avatars/uploads", post(avatars::create_upload))
        .route("/api/posts", get(posts::list).post(posts::create))
        .route(
            "/api/posts/{id}",
            get(posts::get).put(posts::update).delete(posts::delete),
        )
        .route(
            "/api/communities",
            get(communities::list).post(communities::create),
        )
        .route(
            "/api/memberships/{community_id}",
            put(memberships::join).delete(memberships::leave),
        )
        .route("/api/sync/push", post(sync::push))
        .route("/api/sync/pull", post(sync::pull))
        .route("/api/sync/backfill", post(sync::backfill))
        .with_state(state)
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

    use app_core::accounts::Session;

    use super::*;

    const MISSING_ID: &str = "01900000-0000-7000-8000-000000000000";
    const PASSWORD: &str = "correct horse battery";

    /// A router over an in-memory database with Alice signed up and one
    /// post of hers, and a stand-in `index.html`. Every request `send`
    /// makes is hers, with her token. The emails go to `mailer`, where
    /// a test reads the code, and uploads to `storage`, in memory, where
    /// a test puts the file a page would have.
    struct TestRouter {
        router: Router,
        token: String,
        mailer: Mailer,
        storage: Storage,
    }

    async fn test_router() -> (TestRouter, Value) {
        let dist = std::env::temp_dir().join(format!("app_web_test_{}", std::process::id()));
        fs::create_dir_all(&dist).expect("a temp dir");
        fs::write(dist.join("index.html"), "<html>pages</html>").expect("index.html");
        let database = app_core::database::Database::open_sqlite_in_memory()
            .await
            .expect("a database");
        let deployment = Deployment::new(database, app_core::deployment::Role::Server);
        let mailer = Mailer::memory();
        let alice = signed_up(&deployment, &mailer, "alice@example.com").await;
        let post = app_core::posts::create(&deployment, &alice.user, "the first post", &[])
            .await
            .expect("a post");
        let post = app_core::posts::get(&deployment, &alice.user, post.id)
            .await
            .expect("the post");
        let post = serde_json::to_value(post).expect("json");
        let storage = Storage::memory();
        let state = AppState {
            deployment,
            mailer: mailer.clone(),
            storage: Some(storage.clone()),
            rate_limits: RateLimits::new(),
            trust_proxy_headers: false,
        };
        let test_router = TestRouter {
            router: router(&dist, state),
            token: alice.token,
            mailer,
            storage,
        };
        (test_router, post)
    }

    /// A user signed up on the server and finished with their code.
    async fn signed_up(deployment: &Deployment, mailer: &Mailer, email: &str) -> Session {
        let sent = app_core::accounts::sign_up(deployment, mailer, email, PASSWORD)
            .await
            .expect("signed up");
        let code = code_sent_to(mailer, email);
        app_core::accounts::sign_in_with_code(deployment, &sent.pending, &code)
            .await
            .expect("the code matched")
    }

    /// The code the last email to the address carried.
    fn code_sent_to(mailer: &Mailer, email: &str) -> String {
        let mail = mailer.last_sent_to(email).expect("an email");
        app_core::mails::code_in(&mail).expect("a code in it")
    }

    async fn send(
        router: &TestRouter,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        send_as(&router.router, Some(&router.token), method, path, body).await
    }

    async fn send_as(
        router: &Router,
        token: Option<&str>,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let (status, _headers, body) = send_with(router, token, None, method, path, body).await;
        (status, body)
    }

    /// A request with a bearer token, or a `Cookie` header, or neither,
    /// answered with its status, its headers and its body.
    async fn send_with(
        router: &Router,
        token: Option<&str>,
        cookie: Option<&str>,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> (StatusCode, header::HeaderMap, Value) {
        let mut request = Request::builder().method(method).uri(path);
        if let Some(token) = token {
            request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        if let Some(cookie) = cookie {
            request = request.header(header::COOKIE, cookie);
        }
        let request = match body {
            Some(body) => request
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string())),
            None => request.body(Body::empty()),
        }
        .expect("a request");
        let response = router.clone().oneshot(request).await.expect("a response");
        let status = response.status();
        let headers = response.headers().clone();
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
        (status, headers, body)
    }

    async fn get(router: &TestRouter, path: &str) -> (StatusCode, Value) {
        send(router, Method::GET, path, None).await
    }

    #[tokio::test]
    async fn posts_list_as_json() {
        let (router, post) = test_router().await;
        let (status, body) = get(&router, "/api/posts").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, json!([post]));
        assert_eq!(post["owner"]["email"], json!("alice@example.com"));
        assert_eq!(post["communities"], json!([]));
    }

    #[tokio::test]
    async fn users_me_is_the_tokens_user_and_nobody_without_one() {
        let (router, _) = test_router().await;
        let (status, body) = get(&router, "/api/users/me").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["email"], json!("alice@example.com"));

        let (status, body) =
            send_as(&router.router, None, Method::GET, "/api/users/me", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body, json!({ "error": "Sign in first" }));
        let (status, _) = send_as(&router.router, None, Method::GET, "/api/posts", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn avatars_upload_then_update_with_field_errors_as_json() {
        let (router, _) = test_router().await;
        let wrong = json!({ "content_type": "text/html", "size": 0 });
        let (status, body) = send(&router, Method::POST, "/api/avatars/uploads", Some(wrong)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            body["errors"],
            json!({
                "content_type": ["must be a PNG, JPEG, GIF or WebP picture"],
                "size": ["is empty"],
            })
        );

        let png = b"\x89PNG\r\n\x1a\nthe rest".to_vec();
        let picked = json!({ "content_type": "image/png", "size": png.len() });
        let (status, upload) =
            send(&router, Method::POST, "/api/avatars/uploads", Some(picked)).await;
        assert_eq!(status, StatusCode::CREATED);
        let key = upload["key"].as_str().expect("a key").to_string();
        assert_eq!(upload["content_type"], json!("image/png"));

        router.storage.put(&key, png).await.expect("the upload");
        let (status, user) = send(
            &router,
            Method::PUT,
            "/api/avatars",
            Some(json!({ "key": key })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(user["avatar_url"], json!(format!("memory:///{key}")));
        let (_, me) = get(&router, "/api/users/me").await;
        assert_eq!(me["avatar_url"], user["avatar_url"]);
    }

    #[tokio::test]
    async fn accounts_sign_up_then_the_code_sign_in_and_sign_out_over_the_cookie() {
        let (router, _) = test_router().await;
        let bob = json!({ "email": "Bob@example.com", "password": PASSWORD });
        let (status, body) = send_as(
            &router.router,
            None,
            Method::POST,
            "/api/accounts/sign-up",
            Some(bob.clone()),
        )
        .await;
        assert_eq!(status, StatusCode::ACCEPTED);
        let pending = body["pending"].as_str().expect("the pending token");
        let code = code_sent_to(&router.mailer, "bob@example.com");

        let (status, body) = send_as(
            &router.router,
            None,
            Method::POST,
            "/api/accounts/code",
            Some(json!({ "pending": pending, "code": "000000" })),
        )
        .await;
        if code != "000000" {
            assert_eq!(status, StatusCode::UNAUTHORIZED);
            assert!(
                body["error"]
                    .as_str()
                    .unwrap()
                    .starts_with("That code is wrong")
            );
        }
        let (status, _) = send_as(
            &router.router,
            None,
            Method::POST,
            "/api/accounts/code/resend",
            Some(json!({ "pending": pending })),
        )
        .await;
        assert_eq!(status, StatusCode::ACCEPTED);
        let code = code_sent_to(&router.mailer, "bob@example.com");
        let (status, headers, body) = send_with(
            &router.router,
            None,
            None,
            Method::POST,
            "/api/accounts/code",
            Some(json!({ "pending": pending, "code": code })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["user"]["email"], json!("bob@example.com"));
        assert!(body["user"]["confirmed_at"].is_string());
        let set_cookie = headers[header::SET_COOKIE].to_str().expect("a cookie");
        assert!(set_cookie.starts_with(&format!("token={}", body["token"].as_str().unwrap())));
        assert!(set_cookie.contains("HttpOnly"));
        assert!(set_cookie.contains("SameSite=Strict"));
        assert!(set_cookie.contains("Secure"), "no Host, so not localhost");
        let cookie = set_cookie.split(';').next().unwrap().to_string();

        let (status, _, me) = send_with(
            &router.router,
            None,
            Some(&cookie),
            Method::GET,
            "/api/users/me",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(me["email"], json!("bob@example.com"));
        let (_, _, posts) = send_with(
            &router.router,
            None,
            Some(&cookie),
            Method::GET,
            "/api/posts",
            None,
        )
        .await;
        assert_eq!(posts, json!([]), "bob does not see alice's post");

        let (status, headers, _) = send_with(
            &router.router,
            None,
            Some(&cookie),
            Method::DELETE,
            "/api/accounts/sign-out",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        let cleared = headers[header::SET_COOKIE].to_str().expect("a cookie");
        assert!(cleared.starts_with("token=;"), "{cleared}");
        let (status, _, _) = send_with(
            &router.router,
            None,
            Some(&cookie),
            Method::GET,
            "/api/users/me",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);

        let (status, body) = send_as(
            &router.router,
            None,
            Method::POST,
            "/api/accounts/sign-in",
            Some(json!({ "email": "bob@example.com", "password": "wrong horse battery" })),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(
            body,
            json!({ "error": "That email and password do not match" })
        );
        let (status, body) = send_as(
            &router.router,
            None,
            Method::POST,
            "/api/accounts/sign-in",
            Some(bob),
        )
        .await;
        assert_eq!(status, StatusCode::ACCEPTED);
        assert!(body["pending"].is_string());
        let code = code_sent_to(&router.mailer, "bob@example.com");
        let (status, body) = send_as(
            &router.router,
            None,
            Method::POST,
            "/api/accounts/code",
            Some(json!({ "pending": body["pending"], "code": code })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["user"]["email"], json!("bob@example.com"));

        let sent_before = router.mailer.sent().len();
        let (status, body) = send_as(
            &router.router,
            None,
            Method::POST,
            "/api/accounts/sign-up",
            Some(json!({ "email": "bob@example.com", "password": PASSWORD })),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::ACCEPTED,
            "a taken email is answered as a new one"
        );
        assert!(body["pending"].is_string());
        assert_eq!(router.mailer.sent().len(), sent_before + 1);
        assert_eq!(
            router
                .mailer
                .last_sent_to("bob@example.com")
                .unwrap()
                .subject,
            "You already have an account"
        );
        let (status, body) = send_as(
            &router.router,
            None,
            Method::POST,
            "/api/accounts/sign-up",
            Some(json!({ "email": "carol@example.com", "password": "short" })),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            body,
            json!({ "error": "a password needs at least 12 characters" })
        );
    }

    #[tokio::test]
    async fn accounts_routes_are_rate_limited_per_address_per_sign_in_and_per_client() {
        let (router, _) = test_router().await;
        let alice = json!({ "email": "alice@example.com", "password": PASSWORD });
        let mut pending = Value::Null;
        for _ in 0..rate_limits::EMAILS_PER_HOUR_PER_ADDRESS {
            let (status, body) = send_as(
                &router.router,
                None,
                Method::POST,
                "/api/accounts/sign-in",
                Some(alice.clone()),
            )
            .await;
            assert_eq!(status, StatusCode::ACCEPTED);
            pending = body["pending"].clone();
        }
        let (status, body) = send_as(
            &router.router,
            None,
            Method::POST,
            "/api/accounts/sign-in",
            Some(alice),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::TOO_MANY_REQUESTS,
            "the sixth email in an hour"
        );
        assert_eq!(
            body,
            json!({ "error": "Too many attempts; try again in a few minutes" })
        );

        let resend = json!({ "pending": pending });
        let (status, _) = send_as(
            &router.router,
            None,
            Method::POST,
            "/api/accounts/code/resend",
            Some(resend.clone()),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::TOO_MANY_REQUESTS,
            "counted as an email to alice too"
        );
        let (router, _) = test_router().await;
        let (_, body) = send_as(
            &router.router,
            None,
            Method::POST,
            "/api/accounts/sign-in",
            Some(json!({ "email": "alice@example.com", "password": PASSWORD })),
        )
        .await;
        let resend = json!({ "pending": body["pending"] });
        let (status, _) = send_as(
            &router.router,
            None,
            Method::POST,
            "/api/accounts/code/resend",
            Some(resend.clone()),
        )
        .await;
        assert_eq!(status, StatusCode::ACCEPTED);
        let (status, _) = send_as(
            &router.router,
            None,
            Method::POST,
            "/api/accounts/code/resend",
            Some(resend),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::TOO_MANY_REQUESTS,
            "one a minute per sign-in"
        );

        let (router, _) = test_router().await;
        let garbage = json!({ "code": "nope", "verifier": "nope" });
        for _ in 0..rate_limits::ACCOUNT_REQUESTS_PER_MINUTE_PER_IP {
            let (status, _) = send_as(
                &router.router,
                None,
                Method::POST,
                "/api/accounts/device-token",
                Some(garbage.clone()),
            )
            .await;
            assert_eq!(status, StatusCode::UNAUTHORIZED);
        }
        let (status, headers, body) = send_with(
            &router.router,
            None,
            None,
            Method::POST,
            "/api/accounts/device-token",
            Some(garbage),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::TOO_MANY_REQUESTS,
            "the thirty-first in a minute"
        );
        assert!(headers.get(header::RETRY_AFTER).is_some());
        assert_eq!(
            body,
            json!({ "error": "Too many attempts; try again in a few minutes" })
        );
        let (status, _) = send_as(&router.router, None, Method::GET, "/api/users/me", None).await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "other routes are not limited"
        );
    }

    #[tokio::test]
    async fn accounts_a_device_signs_in_through_the_browser_and_exchanges_its_code() {
        let (router, _) = test_router().await;
        let (verifier, challenge) = app_core::accounts::verifier_and_challenge();
        let (status, body) = send_as(
            &router.router,
            None,
            Method::POST,
            "/api/accounts/sign-in",
            Some(json!({ "email": "alice@example.com", "password": PASSWORD })),
        )
        .await;
        assert_eq!(status, StatusCode::ACCEPTED);
        let code = code_sent_to(&router.mailer, "alice@example.com");
        let (status, headers, body) = send_with(
            &router.router,
            None,
            None,
            Method::POST,
            "/api/accounts/code",
            Some(json!({ "pending": body["pending"], "code": code, "challenge": challenge })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(
            headers.get(header::SET_COOKIE).is_none(),
            "no cookie for a device sign-in"
        );
        let device_code = body["code"].as_str().expect("the device code");
        assert!(body.get("token").is_none());

        let (status, _) = send_as(
            &router.router,
            None,
            Method::POST,
            "/api/accounts/device-token",
            Some(json!({ "code": device_code, "verifier": "not the verifier" })),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let (status, headers, session) = send_with(
            &router.router,
            None,
            None,
            Method::POST,
            "/api/accounts/device-token",
            Some(json!({ "code": device_code, "verifier": verifier })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(headers.get(header::SET_COOKIE).is_none());
        assert_eq!(session["user"]["email"], json!("alice@example.com"));
        let token = session["token"].as_str().unwrap();
        let (status, me) = send_as(
            &router.router,
            Some(token),
            Method::GET,
            "/api/users/me",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(me["email"], json!("alice@example.com"));
        let (status, _) = send_as(
            &router.router,
            None,
            Method::POST,
            "/api/accounts/device-token",
            Some(json!({ "code": device_code, "verifier": verifier })),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "a device code works once");
    }

    #[tokio::test]
    async fn communities_create_join_leave_and_a_post_in_one() {
        let (router, _) = test_router().await;
        let (status, community) = send(
            &router,
            Method::POST,
            "/api/communities",
            Some(json!({ "name": "Gardening" })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let community_id = community["id"].as_str().unwrap().to_string();

        let (status, listed) = get(&router, "/api/communities").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(listed[0]["community"], community);
        assert_eq!(listed[0]["membership"]["role"], json!("member"));

        let (status, created) = send(
            &router,
            Method::POST,
            "/api/posts",
            Some(json!({ "body": "roses", "community_ids": [community_id] })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let (_, shown) = get(
            &router,
            &format!("/api/posts/{}", created["id"].as_str().unwrap()),
        )
        .await;
        assert_eq!(shown["communities"][0]["name"], json!("Gardening"));

        let path = format!("/api/memberships/{community_id}");
        let (status, _) = send(&router, Method::DELETE, &path, None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        let (_, listed) = get(&router, "/api/communities").await;
        assert_eq!(listed[0]["membership"], Value::Null);
        let (_, shown) = get(
            &router,
            &format!("/api/posts/{}", created["id"].as_str().unwrap()),
        )
        .await;
        assert_eq!(shown["communities"], json!([]), "left with the member");

        let (status, membership) = send(&router, Method::PUT, &path, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(membership["community_id"], json!(community_id));
        let (_, shown) = get(
            &router,
            &format!("/api/posts/{}", created["id"].as_str().unwrap()),
        )
        .await;
        assert_eq!(shown["communities"][0]["name"], json!("Gardening"));

        let (status, _) = send(
            &router,
            Method::POST,
            "/api/sync/backfill",
            Some(json!({ "community_id": community_id })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let (status, _) = send(
            &router,
            Method::POST,
            "/api/sync/backfill",
            Some(json!({ "community_id": MISSING_ID })),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn a_bearer_token_names_the_user_and_an_unknown_one_is_401() {
        let (router, post) = test_router().await;
        let (status, body) = send_as(
            &router.router,
            Some("nope"),
            Method::GET,
            "/api/posts",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(
            body,
            json!({ "error": "The server did not accept this token" })
        );

        let (status, body) = send_as(
            &router.router,
            None,
            Method::POST,
            "/api/accounts/sign-up",
            Some(json!({ "email": "bob@example.com", "password": PASSWORD })),
        )
        .await;
        assert_eq!(status, StatusCode::ACCEPTED);
        let code = code_sent_to(&router.mailer, "bob@example.com");
        let (status, body) = send_as(
            &router.router,
            None,
            Method::POST,
            "/api/accounts/code",
            Some(json!({ "pending": body["pending"], "code": code })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let token = body["token"].as_str().unwrap();
        let (status, created) = send_as(
            &router.router,
            Some(token),
            Method::POST,
            "/api/posts",
            Some(json!({ "body": "bob's" })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(created["owner_id"], body["user"]["id"]);
        let (_, bobs) = send_as(&router.router, Some(token), Method::GET, "/api/posts", None).await;
        assert_eq!(bobs[0]["post"], created);
        let (_, alices) = get(&router, "/api/posts").await;
        assert_eq!(alices, json!([post]), "alice does not see bob's post");
    }

    #[tokio::test]
    async fn posts_get_as_json() {
        let (router, post) = test_router().await;
        let (status, body) = get(
            &router,
            &format!("/api/posts/{}", post["post"]["id"].as_str().unwrap()),
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
        assert_eq!(posts[0]["post"], created);
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
        let path = format!("/api/posts/{}", post["post"]["id"].as_str().unwrap());
        let (status, updated) = send(
            &router,
            Method::PUT,
            &path,
            Some(json!({ "body": "edited" })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(updated["body"], json!("edited"));
        assert_eq!(updated["inserted_at"], post["post"]["inserted_at"]);

        let (_, read) = get(&router, &path).await;
        assert_eq!(read["post"], updated);
    }

    #[tokio::test]
    async fn posts_delete() {
        let (router, post) = test_router().await;
        let path = format!("/api/posts/{}", post["post"]["id"].as_str().unwrap());
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
            "row_id": post["post"]["id"],
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
        assert_eq!(body["rows"][0]["table"], json!("app_users"));
        assert_eq!(body["rows"][1]["table"], json!("app_posts"));
        assert_eq!(body["rows"][1]["row"]["body"], json!("pushed"));
        assert_eq!(body["next"], Value::Null);
        assert_eq!(body["resync"], json!(false));
        assert!(body["cursor"]["last_synced_at"].is_string());
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
