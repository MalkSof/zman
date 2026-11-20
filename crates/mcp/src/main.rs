//! zman-mcp: local MCP server over stdio. One JSON-RPC message per line in,
//! one per line out. Diagnostics go to stderr, never stdout.

use std::error::Error;
use std::io::{BufRead, Write};

use jiff::tz::TimeZone;
use zman_core::{Config, Lang, Paths, Store};
use zman_mcp::Server;

fn main() {
    if let Err(e) = run() {
        eprintln!("zman-mcp: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let paths = Paths::resolve()?;
    let config = Config::load(&paths.config_file)?;
    // Opening creates an empty database if the daemon has never run, so the
    // tools answer "nothing tracked" instead of failing.
    let store = Store::open(&paths.db_file)?;
    let server = Server {
        source: &store,
        categories: config.categories(Lang::detect()),
        default_tz: TimeZone::system(),
    };

    let mut stdout = std::io::stdout().lock();
    for line in std::io::stdin().lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(reply) = server.handle_text(&line) {
            writeln!(stdout, "{reply}")?;
            stdout.flush()?;
        }
    }
    Ok(())
}
