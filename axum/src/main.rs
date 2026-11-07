//! The web server: `app_core` over a JSON API under `/api`, and the Svelte
//! pages (`../svelte/dist`) at every other path. Listens on `PORT`, 4000
//! when unset, and opens the database `DATABASE_URL` names: a SQLite file,
//! under `data/` beside this crate when unset, or a PostgreSQL database.

mod posts;
mod router;

use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;

use app_core::database::Database;

const DEFAULT_PORT: u16 = 4000;

fn crate_directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The built Svelte pages: `SVELTE_DIST`, else `../svelte/dist` beside this crate.
fn svelte_dist() -> PathBuf {
    env::var_os("SVELTE_DIST")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate_directory().join("../svelte/dist"))
}

fn port() -> Result<u16, String> {
    match env::var("PORT") {
        Ok(value) => value
            .parse()
            .map_err(|_| format!("PORT must be a number, got '{value}'")),
        Err(_) => Ok(DEFAULT_PORT),
    }
}

/// The database `DATABASE_URL` names: `sqlite:<path>`, relative to this
/// crate, or `postgres://...`; the file under `data/` when unset.
async fn open_database() -> Result<Database, String> {
    let url = env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:data/app.db".to_string());
    if let Some(path) = url.strip_prefix("sqlite:") {
        let path = crate_directory().join(path);
        tracing::info!("database {}", path.display());
        return Database::open_sqlite_file(&path)
            .await
            .map_err(|error| error.to_string());
    }
    if url.starts_with("postgres://") || url.starts_with("postgresql://") {
        tracing::info!("database PostgreSQL");
        return Database::open_postgres(&url)
            .await
            .map_err(|error| error.to_string());
    }
    Err(format!(
        "DATABASE_URL must be sqlite:<path> or postgres://..., got '{url}'"
    ))
}

#[tokio::main]
async fn main() -> Result<(), String> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "app_web=info,tower_http=info".into()),
        )
        .init();

    let database = open_database().await?;
    // The sample posts, until the pages can write posts
    if cfg!(debug_assertions) {
        app_core::posts::seed_sample_posts(&database)
            .await
            .map_err(|error| error.to_string())?;
    }

    let address = SocketAddr::from(([127, 0, 0, 1], port()?));
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .map_err(|error| format!("could not listen on {address}: {error}"))?;
    tracing::info!("listening on http://{address}");

    axum::serve(listener, router::router(&svelte_dist(), database))
        .await
        .map_err(|error| format!("server stopped: {error}"))
}
