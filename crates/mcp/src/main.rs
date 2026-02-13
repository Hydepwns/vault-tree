use anyhow::Result;
use std::io::{self, BufRead, Write};

mod server;
mod tools;
mod transport;

use server::McpServer;

fn main() -> Result<()> {
    let mut server = McpServer::new();

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let response = server.handle_request(&line);
        if let Some(resp) = response {
            writeln!(stdout, "{}", resp)?;
            stdout.flush()?;
        }
    }

    Ok(())
}
