//! zman-mcp: local MCP server over stdio. Diagnostics go to stderr.

fn main() {
    if let Err(e) = zman_mcp::serve_stdio() {
        eprintln!("zman-mcp: {e}");
        std::process::exit(1);
    }
}
