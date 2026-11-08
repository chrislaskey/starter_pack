//! The web server: `app_core` over a JSON API under `/api`, the sync
//! endpoints the apps converge with under `/api/sync`, and the Svelte
//! pages (`../svelte/dist`) at every other path. Listens on `PORT`, 4000
//! when unset, and opens the database `DATABASE_URL` names: a SQLite file,
//! under `data/` beside this crate when unset, or a PostgreSQL database.

mod database;
mod posts;
mod router;
mod sync;

use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;

const DEFAULT_PORT: u16 = 4000;

/// The built Svelte pages: `SVELTE_DIST`, else `../svelte/dist` beside this crate.
fn svelte_dist() -> PathBuf {
    env::var_os("SVELTE_DIST")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../svelte/dist"))
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
                .unwrap_or_else(|_| "app_web=info,tower_http=info".into()),
        )
        .init();

    let deployment = database::open().await?;

    let address = SocketAddr::from(([127, 0, 0, 1], port()?));
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .map_err(|error| format!("could not listen on {address}: {error}"))?;
    tracing::info!("listening on http://{address}");

    axum::serve(listener, router::router(&svelte_dist(), deployment))
        .await
        .map_err(|error| format!("server stopped: {error}"))
}
