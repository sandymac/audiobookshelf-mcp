# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

An MCP server written in Rust that bridges AI assistants to a running [Audiobookshelf](https://www.audiobookshelf.org/) instance. Built with the [Model Context Protocol Rust SDK](https://github.com/modelcontextprotocol/rust-sdk), it exposes tools for querying libraries, managing audiobooks/podcasts, and tracking listening progress via the Audiobookshelf REST API.

Supports both **stdio** (Claude Desktop) and **HTTP/SSE** (remote/agentic) transports. Includes tiered safety gates to guard against LLM hallucination.

## Tech Stack

Rust for the code
Cargo for package management and build system

## Dependencies

| Crate | Purpose |
|---|---|
| `rmcp` | Official MCP Rust SDK — stdio and HTTP/SSE transports; features: `server`, `transport-io`, `transport-streamable-http-server`, `schemars` |
| `tokio` | Async runtime (all features) |
| `reqwest` | HTTP client for Audiobookshelf REST API (features: `json`) |
| `serde` / `serde_json` | Serialization |
| `anyhow` | Flexible error propagation |
| `thiserror` | Structured error types |
| `axum` | HTTP server for the HTTP/SSE transport |
| `tower-http` | CORS and tracing middleware layers for axum |
| `tracing` | Logging |
| `tracing-subscriber` | Log output — **must write to stderr, never stdout**; stdout is reserved for MCP JSON-RPC framing |
| `clap` | CLI args (server URL, API token, transport selection, tool enable/disable); credentials also via env vars |
| `subtle` | Constant-time Bearer token comparison for HTTP transport |

## Architecture

```
MCP Client (Claude Desktop, agentic frameworks, etc.) <--MCP--> audiobookshelf-mcp <--REST API--> Audiobookshelf
```

The server supports two MCP transports:
- **stdio** — for local use with Claude Desktop and similar clients
- **HTTP/SSE** — for remote use with network-accessible clients; MCP clients connect to `http://<host>:<port>/mcp`. Protected by optional Bearer token authentication (`--api-token`).

### Audiobookshelf API

Audiobookshelf exposes a REST API authenticated via an API token. API keys are created by admin users in Settings → Users → API Keys (requires Audiobookshelf v2.26.0+). Each key acts on behalf of a specific user and can have an optional expiration date. All requests include an `Authorization: Bearer <token>` header. The full API surface is documented in `docs/openapi.json` in the Audiobookshelf repository.

### HTTP Client Setup

The `reqwest::Client` is constructed once at startup with the Authorization header pre-configured and Arc-wrapped for sharing across async tool calls:

```rust
let mut headers = HeaderMap::new();
headers.insert(header::AUTHORIZATION, format!("Bearer {api_token}"));
let client = reqwest::Client::builder()
    .default_headers(headers)
    .build()?;
```

### Tool Safety Gates

Tools have two default states. Mutating tools are **disabled by default** to guard against LLM hallucination. Enabled/disabled via `--enable-tool <name>` / `--disable-tool <name>` (repeatable; `--disable-tool` always wins). `--list-tools` prints all tools with defaults and exits without requiring credentials. Disabled tools are removed from the `ToolRouter` at startup via `remove_route()` and are invisible to the LLM.

### Tool Registration Pattern (rmcp macros)

```rust
#[derive(Clone)]
pub struct AbsServer {
    client: Arc<AbsClient>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl AbsServer {
    #[tool]
    pub async fn my_tool(&self, Parameters(params): Parameters<MyToolParams>) -> Result<String, String> { ... }
}

#[tool_handler]
impl ServerHandler for AbsServer {}
```

Parameter structs use `#[derive(Deserialize, schemars::JsonSchema)]`; doc comments become JSON Schema descriptions. Tool methods return `Result<String, String>` — `Ok` is the result text sent to the LLM, `Err` is an actionable error message.

**Critical rmcp ≥1.3.0 requirement**: Tool arguments must be wrapped in `Parameters<T>` — plain `T` does not implement `FromContextPart` and will cause a compile-time `IntoToolRoute` bound error. Use destructuring pattern:
```rust
pub async fn my_tool(&self, Parameters(params): Parameters<MyParams>) -> Result<String, String>
```
`remove_route` is now in-place (returns `()`), not chainable — use a `for` loop instead of `fold`.

### Transport Setup Pattern

```rust
// Stdio (default)
let service = server.serve(rmcp::transport::stdio()).await?;
service.waiting().await?;

// HTTP — new server instance per session via factory
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};
let mcp_service = StreamableHttpService::new(
    move || Ok(AbsServer::new(client.clone(), enabled.clone())),
    Arc::new(LocalSessionManager::default()),
    StreamableHttpServerConfig::default(),
);
let app = Router::new()
    .nest_service("/mcp", mcp_service)
    .layer(middleware::from_fn(auth_middleware_closure))
    .layer(CorsLayer::permissive())
    .layer(TraceLayer::new_for_http());
axum::serve(listener, app).await?;
```

### Error Handling

HTTP error codes from Audiobookshelf get `[Hint: ...]` suffixes for LLM recovery:
- 401/403 → hint to verify the API token in user settings
- 404 → hint that the library item or resource ID may be wrong
- 429 → hint to wait before retrying

### Logging

`tracing_subscriber` configured to write to `stderr` explicitly. `RUST_LOG` controls verbosity (default: INFO). Startup logs list enabled/disabled tools and the server URL.

## MCP Tools

| Tool | Default | API Endpoint | Description |
|---|---|---|---|
| `list_libraries` | enabled | `GET /api/libraries` | List all libraries with IDs, names, and media types |
| `search_library` | enabled | `GET /api/libraries/{id}/search` | Search by title, author, narrator, series, or ISBN |
| `get_library_items` | enabled | `GET /api/libraries/{id}/items` | Paginated item list with sorting |
| `get_item` | enabled | `GET /api/items/{id}` | Full item details including progress, chapters, files |
| `get_in_progress` | enabled | `GET /api/me/items-in-progress` | Items currently in progress for the authenticated user |
| `get_listening_stats` | enabled | `GET /api/me/listening-stats` | Total time, per-day breakdown, most-listened items |
| `get_recent_sessions` | enabled | `GET /api/me/listening-sessions` | Recent playback sessions, paginated |
| `get_metadata_object` | enabled | `GET /api/items/{id}/metadata-object` | Raw metadata object for one library item |
| `find_items_missing_metadata` | enabled | `GET /api/libraries/{id}/items` | Scan one minified page for items missing common metadata fields |
| `update_progress` | **disabled** | `PATCH /api/me/progress/{id}` | Record playback position or mark item finished |
| `create_bookmark` | **disabled** | `POST /api/me/item/{id}/bookmark` | Create a bookmark at a playback position |
| `delete_bookmark` | **disabled** | `DELETE /api/me/item/{id}/bookmark/{time}` | Delete a bookmark by its exact time value |
| `quick_match_item` | **disabled** | `POST /api/items/{id}/match` | Quick-match metadata for one library item |
| `batch_quick_match_items` | **disabled** | `POST /api/items/batch/quickmatch` | Quick-match metadata for multiple library items |
| `batch_update_metadata` | **disabled** | `POST /api/items/batch/update` | Batch-update item metadata objects |

Mutating tools are disabled by default as they mutate server state. Enable with e.g. `--enable-tool update_progress` or `--enable-tool batch_update_metadata`.

Metadata mutation payloads follow Audiobookshelf controller semantics: `quick_match_item` sends `overrideCover` and `overrideDetails` at the top level, `batch_quick_match_items` sends them under `options`, and `batch_update_metadata` sends an array of `{ id, mediaPayload: { metadata } }` entries.

## Configuration (CLI flags / env vars)

| Flag | Env Var | Purpose |
|---|---|---|
| `--server-url` | `ABS_SERVER_URL` | Audiobookshelf server base URL (e.g. `http://localhost:13378`) |
| `--api-token` | `ABS_API_TOKEN` | API token from Audiobookshelf user settings |
| `--transport` | — | `stdio` (default) or `http` |
| `--http-bind` | — | Bind address for HTTP transport (default: `127.0.0.1:8080`) |
| `--http-api-token` | `ABS_HTTP_API_TOKEN` | Bearer token protecting the HTTP/SSE MCP endpoint |
| `--enable-tool <name>` | — | Enable a tool by name (repeatable) |
| `--disable-tool <name>` | — | Disable a tool by name (repeatable, always wins) |
| `--list-tools` | — | Print all tools with defaults and exit |
| `--test-connection` | — | Verify API token against `/api/me`, then exit |

## Commands

```bash
# Build
cargo build

# Build (optimized release)
cargo build --release

# Run (stdio transport)
cargo run -- --server-url http://localhost:13378 --api-token <TOKEN>

# Run tests
cargo test

# Run a single test
cargo test <test_name>

# List tools without credentials
cargo run -- --list-tools

# Verify server connectivity
cargo run -- --server-url http://localhost:13378 --api-token <TOKEN> --test-connection

# Build docs
cargo doc --open
```

## File Structure

| Path | Purpose |
|---|---|
| `Cargo.toml` | Manifest — dependencies, metadata |
| `src/main.rs` | CLI arg parsing, transport selection, server startup, HTTP auth middleware |
| `src/abs/mod.rs` | Audiobookshelf HTTP client — client construction, base URL, error enrichment |
| `src/tools/mod.rs` | All MCP tool implementations, `TOOL_REGISTRY`, parameter structs |
