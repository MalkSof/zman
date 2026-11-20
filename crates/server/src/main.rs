//! zman-server. Configuration comes from the environment:
//!
//! - `DATABASE_URL`: Postgres connection string, or `memory:` to keep
//!   everything in RAM (for trying the server out; data is lost on exit).
//! - `ZMAN_PUBLIC_URL`: the address clients reach this server at, e.g.
//!   `https://zman.example.com`. Defaults to `http://localhost:PORT`.
//! - `ZMAN_BIND`: address to listen on. Default `0.0.0.0:8787`.
//! - `ZMAN_ALLOW_REGISTRATION`: `false` to stop new accounts. Default `true`.

use std::error::Error;

use zman_server::db::Db;
use zman_server::{AppState, app};

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("zman-server: {e}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn Error>> {
    let env = |name: &str| std::env::var(name).ok().filter(|v| !v.trim().is_empty());

    let bind = env("ZMAN_BIND").unwrap_or_else(|| "0.0.0.0:8787".to_string());
    let port = bind.rsplit(':').next().unwrap_or("8787");
    let public_url = env("ZMAN_PUBLIC_URL").unwrap_or_else(|| format!("http://localhost:{port}"));
    let allow_registration = env("ZMAN_ALLOW_REGISTRATION").is_none_or(|v| !v.eq_ignore_ascii_case("false"));

    let database_url = env("DATABASE_URL").ok_or("DATABASE_URL is not set (use a postgres:// URL, or memory:)")?;
    let db = if database_url == "memory:" {
        eprintln!("zman-server: using in-memory storage, data will be lost on exit");
        Db::memory()
    } else {
        Db::connect(&database_url).await?
    };

    let state = AppState { db, public_url: public_url.trim_end_matches('/').to_string(), allow_registration };
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    println!("zman-server listening on {bind}, public URL {}", state.public_url);
    axum::serve(listener, app(state))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
