//! The web server: `app_core` over a JSON API under `/api`, the sync
//! endpoints the apps converge with under `/api/sync`, and the Svelte
//! pages (`../svelte/dist`) at every other path. Every request is from a
//! user, see `current_user`. Listens on `PORT`, 4000 when unset, and
//! opens the database `DATABASE_URL` names: a SQLite file, under `data/`
//! beside this crate when unset, or a PostgreSQL database. The email it
//! sends goes where `MAILER` says, see `mailer`, and its accounts routes
//! are rate limited, see `rate_limits`.
//!
//! With arguments it runs a command on that database instead and exits:
//! `purge` removes what was deleted longer ago than the retention
//! window, for a daily cron job, and prints what went.

mod accounts;
mod api_error;
mod communities;
mod current_user;
mod database;
mod mailer;
mod memberships;
mod posts;
mod rate_limits;
mod router;
mod sync;
mod users;

use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use crate::rate_limits::RateLimits;
use crate::router::AppState;

const DEFAULT_PORT: u16 = 4000;

/// The built Svelte pages: `SVELTE_DIST`, else `../svelte/dist` beside this crate.
fn svelte_dist() -> PathBuf {
    env::var_os("SVELTE_DIST")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../svelte/dist"))
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
    let state = AppState {
        deployment,
        mailer,
        rate_limits,
        trust_proxy_headers: trust_proxy_headers(),
    };

    let address = SocketAddr::from(([127, 0, 0, 1], port()?));
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .map_err(|error| format!("could not listen on {address}: {error}"))?;
    tracing::info!("listening on http://{address}");

    let app = router::router(&svelte_dist(), state);
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
        ["purge"] => {
            let report = app_core::purge::run(deployment, app_core::deployment::now())
                .await
                .map_err(|error| error.to_string())?;
            println!("purged up to {}", report.horizon);
            for (table, count) in &report.removed {
                println!("{table} {count}");
            }
            Ok(())
        }
        _ => Err("commands: purge".to_string()),
    }
}
