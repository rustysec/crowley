//! crowley — an MCP server that fetches web content as markdown via `crwl`.

mod config;
mod crwl;
mod server;

use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Parser;

use crate::config::Cli;
use crate::server::CrowleyServer;

/// Set up stderr-only logging so stdout stays a clean MCP transport.
fn init_tracing(verbose: bool) {
    let filter = if verbose {
        tracing_subscriber::EnvFilter::new("crowley=debug,rmcp=info")
    } else {
        tracing_subscriber::EnvFilter::new("crowley=info,rmcp=warn")
    };
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}

/// Wait for SIGINT (ctrl-c) or SIGTERM.
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut terminate = signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = terminate.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let mut config = config::Config::load(cli.config.as_deref())?;
    config.apply_cli(&cli);

    if cli.print_config {
        let rendered = config.to_toml()?;
        print!("{rendered}");
        return Ok(());
    }

    init_tracing(config.verbose);

    tracing::info!(
        crwl_bin = %config.crwl_bin,
        output = %config.output_format.as_str(),
        timeout_secs = config.timeout_secs,
        "crowley starting"
    );

    let server = CrowleyServer::new(Arc::new(config));
    // Block until the client sends `initialize` or the transport fails.
    let running = rmcp::serve_server(server, rmcp::transport::stdio())
        .await
        .context("failed to serve MCP over stdio")?;
    tracing::info!("crowley serving MCP over stdio");

    // Wait for the service loop to end (client disconnect, EOF on stdin, or a
    // shutdown signal). Dropping the pending future on shutdown cancels the
    // running service via its drop guard.
    let quit = running.waiting();
    tokio::pin!(quit);
    tokio::select! {
        quit = quit => {
            tracing::info!("MCP service ended: {quit:?}");
        }
        _ = shutdown_signal() => {
            tracing::info!("shutdown signal received, exiting");
        }
    }

    Ok(())
}
