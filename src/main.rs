// Copyright (c) 2026 Sandy McArthur, Jr.
// SPDX-License-Identifier: MIT

use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    extract::Request,
    http::{header, StatusCode},
    middleware::{self, Next},
    response::IntoResponse,
    Router,
};
use clap::Parser;
use rmcp::ServiceExt;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

mod abs;
mod tools;

use abs::AbsClient;
use tools::{AbsServer, TOOL_REGISTRY};

#[derive(Parser, Debug)]
#[command(name = "audiobookshelf-mcp", about = "MCP server for Audiobookshelf")]
struct Args {
    /// Audiobookshelf server base URL (e.g. http://localhost:13378)
    #[arg(long, env = "ABS_SERVER_URL")]
    server_url: Option<String>,

    /// Audiobookshelf API token — generate one in Settings → Users → API Keys
    #[arg(long, env = "ABS_API_TOKEN")]
    api_token: Option<String>,

    /// MCP transport: stdio (default, for Claude Desktop) or http (for remote clients)
    #[arg(long, default_value = "stdio")]
    transport: Transport,

    /// Bind address for the HTTP transport
    #[arg(long, default_value = "127.0.0.1:8080")]
    http_bind: SocketAddr,

    /// Bearer token protecting the HTTP/SSE MCP endpoint (recommended for HTTP transport)
    #[arg(long, env = "ABS_HTTP_API_TOKEN")]
    http_api_token: Option<String>,

    /// Allow unauthenticated HTTP when binding to a non-loopback address
    #[arg(long)]
    allow_unauthenticated_http: bool,

    /// Enable a tool by name — repeatable, processed before --disable-tool
    #[arg(long = "enable-tool", value_name = "NAME")]
    enable_tools: Vec<String>,

    /// Disable a tool by name — repeatable, always takes precedence over --enable-tool
    #[arg(long = "disable-tool", value_name = "NAME")]
    disable_tools: Vec<String>,

    /// Print all tools with their default state and exit (no credentials required)
    #[arg(long)]
    list_tools: bool,

    /// Call /api/me to verify the API token then exit
    #[arg(long)]
    test_connection: bool,
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum Transport {
    Stdio,
    Http,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let args = Args::parse();

    if args.list_tools {
        println!("{:<30} {:<10} GROUP", "TOOL", "DEFAULT");
        println!("{}", "-".repeat(55));
        for (name, group, enabled) in TOOL_REGISTRY {
            println!(
                "{:<30} {:<10} {}",
                name,
                if *enabled { "enabled" } else { "disabled" },
                group
            );
        }
        return Ok(());
    }

    let server_url = args
        .server_url
        .ok_or_else(|| anyhow::anyhow!("--server-url / ABS_SERVER_URL is required"))?;
    let api_token = args
        .api_token
        .ok_or_else(|| anyhow::anyhow!("--api-token / ABS_API_TOKEN is required"))?;

    // Build enabled set from registry defaults, apply --enable-tool, then --disable-tool wins.
    let mut enabled: HashSet<String> = TOOL_REGISTRY
        .iter()
        .filter(|(_, _, default)| *default)
        .map(|(name, _, _)| name.to_string())
        .collect();

    let known: HashSet<&str> = TOOL_REGISTRY.iter().map(|(n, _, _)| *n).collect();
    for name in &args.enable_tools {
        if !known.contains(name.as_str()) {
            tracing::warn!(tool = %name, "unknown tool name in --enable-tool");
        }
        enabled.insert(name.clone());
    }
    for name in &args.disable_tools {
        enabled.remove(name);
    }

    let disabled: Vec<&str> = TOOL_REGISTRY
        .iter()
        .map(|(n, _, _)| *n)
        .filter(|n| !enabled.contains(*n))
        .collect();
    tracing::info!(enabled = ?enabled.iter().collect::<std::collections::BTreeSet<_>>(), "enabled tools");
    if !disabled.is_empty() {
        tracing::info!(disabled = ?disabled, "disabled tools");
    }

    let client = Arc::new(AbsClient::new(&server_url, &api_token)?);

    if args.test_connection {
        client.test_connection().await?;
        println!("Connection successful.");
        return Ok(());
    }

    match args.transport {
        Transport::Stdio => {
            let server = AbsServer::new(client, enabled);
            let service = server.serve(rmcp::transport::stdio()).await?;
            service.waiting().await?;
        }

        Transport::Http => {
            if args.http_api_token.is_none() {
                if !args.http_bind.ip().is_loopback() && !args.allow_unauthenticated_http {
                    anyhow::bail!(
                        "refusing unauthenticated HTTP bind to {}. Provide --http-api-token / \
                         ABS_HTTP_API_TOKEN or pass --allow-unauthenticated-http explicitly.",
                        args.http_bind
                    );
                }
                if args.http_bind.ip().is_loopback() {
                    tracing::warn!(
                        "HTTP transport started on loopback without --http-api-token: \
                         the /mcp endpoint is unauthenticated"
                    );
                } else {
                    tracing::warn!(
                        "HTTP transport started on a non-loopback address without \
                         --http-api-token because --allow-unauthenticated-http was set: \
                         the /mcp endpoint is unauthenticated"
                    );
                }
            }
            let http_token: Option<Arc<String>> = args.http_api_token.map(Arc::new);

            let mcp_service = {
                use rmcp::transport::streamable_http_server::{
                    session::local::LocalSessionManager, StreamableHttpServerConfig,
                    StreamableHttpService,
                };
                StreamableHttpService::new(
                    {
                        let client = client.clone();
                        let enabled = enabled.clone();
                        move || Ok(AbsServer::new(client.clone(), enabled.clone()))
                    },
                    Arc::new(LocalSessionManager::default()),
                    StreamableHttpServerConfig::default(),
                )
            };

            let app = Router::new()
                .nest_service("/mcp", mcp_service)
                .layer(middleware::from_fn({
                    let token = http_token.clone();
                    move |req: Request, next: Next| {
                        let token = token.clone();
                        async move {
                            if let Some(expected) = token {
                                let provided = req
                                    .headers()
                                    .get(header::AUTHORIZATION)
                                    .and_then(|v| v.to_str().ok())
                                    .and_then(|v| v.strip_prefix("Bearer "));
                                let ok: bool = provided
                                    .map(|t| {
                                        use subtle::ConstantTimeEq;
                                        bool::from(t.as_bytes().ct_eq(expected.as_bytes()))
                                    })
                                    .unwrap_or(false);
                                if !ok {
                                    return StatusCode::UNAUTHORIZED.into_response();
                                }
                            }
                            next.run(req).await
                        }
                    }
                }))
                .layer(CorsLayer::permissive())
                .layer(TraceLayer::new_for_http());

            let listener = tokio::net::TcpListener::bind(args.http_bind).await?;
            tracing::info!(
                "HTTP/SSE transport listening — connect MCP clients to http://{}/mcp",
                args.http_bind
            );
            axum::serve(listener, app).await?;
        }
    }

    Ok(())
}
