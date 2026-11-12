//! The web server: `app_core` over a JSON API under `/api`, the sync
//! endpoints the apps converge with under `/api/sync`, the public website's
//! pages, see `public`, and the Svelte pages (`../svelte/dist`) at every
//! other path. Every request is from a
//! user, see `current_user`. Listens on `PORT`, 4000 when unset, and
//! opens the database `DATABASE_URL` names: a SQLite file, under `data/`
//! beside this crate when unset, or a PostgreSQL database. The email it
//! sends goes where `MAILER` says, see `mailer`, pictures go to the bucket
//! `STORAGE_BUCKET` names and files' bytes to `STORAGE_PRIVATE_BUCKET`'s,
//! see `storage`, and its accounts routes are rate limited, see
//! `rate_limits`. Stripe's webhook needs `STRIPE_WEBHOOK_SECRET`, see
//! `stripe_events`.
//!
//! It runs the recurring jobs as they come due, see `jobs`. With
//! arguments it runs a command on that database instead and exits:
//! `jobs run` runs the jobs due, `jobs run <name>` that job now, and
//! `ops staff create <email>` makes that account staff, see `ops`.

mod accounts;
mod api_error;
mod avatars;
mod communities;
mod current_community;
mod current_user;
mod database;
mod files;
mod jobs;
mod mailer;
mod memberships;
mod ops;
mod posts;
mod public;
mod rate_limits;
mod router;
mod storage;
mod stripe_events;
mod sync;

use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use app_core::deployment::FileStore;

use crate::rate_limits::RateLimits;
use crate::router::AppState;

const DEFAULT_PORT: u16 = 4000;

/// The built Svelte pages: `SVELTE_DIST`, else `../svelte/dist` beside this crate.
fn svelte_dist() -> PathBuf {
    env::var_os("SVELTE_DIST")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../svelte/dist"))
}

/// The built ops pages: `SVELTE_OPS_DIST`, else `../svelte/dist-ops`
/// beside this crate.
fn svelte_ops_dist() -> PathBuf {
    env::var_os("SVELTE_OPS_DIST")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../svelte/dist-ops"))
}

/// Whether the client IP is the reverse proxy's `X-Forwarded-For`
/// header, `TRUST_PROXY_HEADERS=1`; off unless the proxy is the only way
/// in, see `rate_limits`.
fn trust_proxy_headers() -> bool {
    matches!(
        env::var("TRUST_PROXY_HEADERS").as_deref(),
        Ok("1") | Ok("true") | Ok("yes")
    )
}

/// How many trial users the server holds at once, `TRIAL_USERS_MAX`;
/// unset or 0, there are no trials.
fn trial_users_max() -> Result<router::TrialUsersMax, String> {
    match env::var("TRIAL_USERS_MAX") {
        Ok(max) => max
            .parse()
            .map(router::TrialUsersMax)
            .map_err(|_| format!("TRIAL_USERS_MAX must be a number, not '{max}'")),
        Err(_) => Ok(router::TrialUsersMax(0)),
    }
}

fn port() -> Result<u16, String> {
    match env::var("PORT") {
        Ok(value) => value
            .parse()
            .map_err(|_| format!("PORT must be a number, got '{value}'")),
        Err(_) => Ok(DEFAULT_PORT),
    }
}

#[tokio::main]
async fn main() -> Result<(), String> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "app_web=info,app_core=info,tower_http=info".into()),
        )
        .init();

    let deployment = database::open().await?;
    let mailer = mailer::open()?;
    let deployment = match storage::open_public()? {
        Some(public) => deployment.with_avatar_bucket(public),
        None => deployment,
    };
    let deployment = match storage::open_private()? {
        Some(private) => deployment.with_file_store(FileStore::Bucket(private)),
        None => deployment,
    };
    let arguments: Vec<String> = env::args().skip(1).collect();
    if !arguments.is_empty() {
        return command(&deployment, &arguments).await;
    }

    let rate_limits = RateLimits::new();
    let forgetting = rate_limits.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(60 * 60)).await;
            forgetting.forget_old();
        }
    });
    jobs::spawn(deployment.clone());
    let state = AppState {
        deployment,
        mailer,
        rate_limits,
        trust_proxy_headers: trust_proxy_headers(),
        trial_users_max: trial_users_max()?,
        stripe_signing_secret: stripe_events::open(),
    };

    let address = SocketAddr::from(([127, 0, 0, 1], port()?));
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .map_err(|error| format!("could not listen on {address}: {error}"))?;
    tracing::info!("listening on http://{address}");

    let app = router::router(&svelte_dist(), &svelte_ops_dist(), state);
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .map_err(|error| format!("server stopped: {error}"))
}

/// A command on the database, run instead of the server.
async fn command(
    deployment: &app_core::deployment::Deployment,
    arguments: &[String],
) -> Result<(), String> {
    let words: Vec<&str> = arguments.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["jobs", rest @ ..] => jobs::command(deployment, rest).await,
        ["ops", rest @ ..] => ops::command(deployment, rest).await,
        _ => Err(
            "commands: jobs run, jobs run <name>, ops staff create <email>, ops staff delete <email>"
                .to_string(),
        ),
    }
}
