// Copyright (c) 2026 Sandy McArthur, Jr.
// SPDX-License-Identifier: MIT

use std::collections::HashSet;
use std::sync::Arc;

use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_handler, tool_router, ServerHandler};
use serde::Deserialize;

use crate::abs::AbsClient;

/// Registry of all tools: (name, group, default_enabled).
/// Used for --list-tools output and to build the enabled set at startup.
pub const TOOL_REGISTRY: &[(&str, &str, bool)] = &[
    ("list_libraries",      "default",  true),
    ("search_library",      "default",  true),
    ("get_library_items",   "default",  true),
    ("get_item",            "default",  true),
    ("get_in_progress",     "default",  true),
    ("get_listening_stats", "default",  true),
    ("get_recent_sessions", "default",  true),
    ("update_progress",     "progress", false),
    ("create_bookmark",     "progress", false),
    ("delete_bookmark",     "progress", false),
];

#[derive(Clone)]
pub struct AbsServer {
    client: Arc<AbsClient>,
    tool_router: ToolRouter<Self>,
}

impl AbsServer {
    pub fn new(client: Arc<AbsClient>, enabled_tools: HashSet<String>) -> Self {
        let mut tool_router = Self::tool_router();
        for (name, _, _) in TOOL_REGISTRY {
            if !enabled_tools.contains(*name) {
                tool_router.remove_route(name);
            }
        }
        Self { client, tool_router }
    }
}

// ── Parameter structs ─────────────────────────────────────────────────────────

#[derive(Deserialize, schemars::JsonSchema)]
pub struct SearchLibraryParams {
    /// Library ID to search within. Obtain IDs from `list_libraries`.
    pub library_id: String,
    /// Search query — matches title, author, narrator, series, or ISBN.
    pub query: String,
    /// Maximum number of results to return (default: 20).
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct GetLibraryItemsParams {
    /// Library ID. Obtain from `list_libraries`.
    pub library_id: String,
    /// Sort field. Examples: "media.metadata.title", "media.metadata.authorName", "addedAt", "duration", "size".
    #[serde(default)]
    pub sort: Option<String>,
    /// Sort descending when true, ascending when false (default: false).
    #[serde(default)]
    pub desc: bool,
    /// Page number, 0-based (default: 0).
    #[serde(default)]
    pub page: Option<u32>,
    /// Items per page, max 100 (default: 20).
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct GetItemParams {
    /// Library item ID. Obtain from `search_library` or `get_library_items`.
    pub item_id: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct GetRecentSessionsParams {
    /// Sessions per page (default: 10).
    #[serde(default)]
    pub items_per_page: Option<u32>,
    /// Page number, 0-based (default: 0).
    #[serde(default)]
    pub page: Option<u32>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdateProgressParams {
    /// Library item ID.
    pub item_id: String,
    /// Current playback position in seconds.
    pub current_time: f64,
    /// Total duration in seconds. Provide when known — used to compute progress percentage.
    #[serde(default)]
    pub duration: Option<f64>,
    /// Mark the item as fully finished.
    #[serde(default)]
    pub is_finished: bool,
    /// Podcast episode ID — required when updating a podcast episode, omit for audiobooks.
    #[serde(default)]
    pub episode_id: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateBookmarkParams {
    /// Library item ID.
    pub item_id: String,
    /// Playback position in seconds.
    pub time: f64,
    /// Descriptive title for the bookmark.
    pub title: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct DeleteBookmarkParams {
    /// Library item ID.
    pub item_id: String,
    /// Position in seconds of the bookmark to delete — must exactly match an existing bookmark's `time` value.
    pub time: f64,
}

// ── Tools ─────────────────────────────────────────────────────────────────────

#[tool_router]
impl AbsServer {
    /// List all libraries on the Audiobookshelf server.
    /// Returns each library's ID, name, media type (book or podcast), and folder paths.
    /// Use the library IDs with other tools to browse or search content.
    #[tool]
    pub async fn list_libraries(&self) -> Result<String, String> {
        self.client
            .get("/libraries")
            .await
            .map(|v| serde_json::to_string_pretty(&v).unwrap_or_default())
            .map_err(|e| e.to_string())
    }

    /// Search a library for audiobooks, podcasts, authors, series, narrators, or tags.
    /// Results are grouped by type (book/podcast, authors, series, narrators, tags).
    /// Use `list_libraries` first to get a library ID.
    #[tool]
    pub async fn search_library(
        &self,
        Parameters(params): Parameters<SearchLibraryParams>,
    ) -> Result<String, String> {
        let limit = params.limit.unwrap_or(20);
        self.client
            .get_with_params(
                &format!("/libraries/{}/search", params.library_id),
                &[("q", params.query), ("limit", limit.to_string())],
            )
            .await
            .map(|v| serde_json::to_string_pretty(&v).unwrap_or_default())
            .map_err(|e| e.to_string())
    }

    /// Get a paginated list of items from a library with optional sorting.
    /// Returns item IDs, titles, authors, duration, and progress for each item.
    /// Use `get_item` for the full details of a specific item.
    #[tool]
    pub async fn get_library_items(
        &self,
        Parameters(params): Parameters<GetLibraryItemsParams>,
    ) -> Result<String, String> {
        let limit = params.limit.unwrap_or(20).min(100);
        let page = params.page.unwrap_or(0);
        let mut query: Vec<(&str, String)> = vec![
            ("limit", limit.to_string()),
            ("page", page.to_string()),
            ("desc", if params.desc { "1".into() } else { "0".into() }),
            ("minified", "1".into()),
        ];
        if let Some(sort) = params.sort {
            query.push(("sort", sort));
        }
        self.client
            .get_with_params(&format!("/libraries/{}/items", params.library_id), &query)
            .await
            .map(|v| serde_json::to_string_pretty(&v).unwrap_or_default())
            .map_err(|e| e.to_string())
    }

    /// Get full details for a specific library item: metadata, chapters, audio files,
    /// authors, series, and current listening progress.
    /// Use `search_library` or `get_library_items` to find item IDs.
    #[tool]
    pub async fn get_item(
        &self,
        Parameters(params): Parameters<GetItemParams>,
    ) -> Result<String, String> {
        self.client
            .get_with_params(
                &format!("/items/{}", params.item_id),
                &[("expanded", "1".into()), ("include", "progress".into())],
            )
            .await
            .map(|v| serde_json::to_string_pretty(&v).unwrap_or_default())
            .map_err(|e| e.to_string())
    }

    /// Get all audiobooks and podcast episodes currently in progress for the authenticated user.
    /// Shows each item's current position, progress percentage, and time remaining.
    #[tool]
    pub async fn get_in_progress(&self) -> Result<String, String> {
        self.client
            .get("/me/items-in-progress")
            .await
            .map(|v| serde_json::to_string_pretty(&v).unwrap_or_default())
            .map_err(|e| e.to_string())
    }

    /// Get listening statistics for the authenticated user: total time, time per day of week,
    /// daily breakdown, and most-listened items.
    #[tool]
    pub async fn get_listening_stats(&self) -> Result<String, String> {
        self.client
            .get("/me/listening-stats")
            .await
            .map(|v| serde_json::to_string_pretty(&v).unwrap_or_default())
            .map_err(|e| e.to_string())
    }

    /// Get recent playback sessions for the authenticated user, paginated.
    /// Each session shows the item played, start/end time, and minutes listened.
    #[tool]
    pub async fn get_recent_sessions(
        &self,
        Parameters(params): Parameters<GetRecentSessionsParams>,
    ) -> Result<String, String> {
        let per_page = params.items_per_page.unwrap_or(10);
        let page = params.page.unwrap_or(0);
        self.client
            .get_with_params(
                "/me/listening-sessions",
                &[
                    ("itemsPerPage", per_page.to_string()),
                    ("page", page.to_string()),
                ],
            )
            .await
            .map(|v| serde_json::to_string_pretty(&v).unwrap_or_default())
            .map_err(|e| e.to_string())
    }

    /// Update listening progress for an audiobook or podcast episode.
    /// Use to record playback position, mark an item finished, or reset progress.
    /// For podcasts, provide episode_id. Enable with: --enable-tool update_progress
    #[tool]
    pub async fn update_progress(
        &self,
        Parameters(params): Parameters<UpdateProgressParams>,
    ) -> Result<String, String> {
        let mut body = serde_json::json!({
            "currentTime": params.current_time,
            "isFinished": params.is_finished,
        });
        if let Some(duration) = params.duration {
            body["duration"] = serde_json::json!(duration);
            if duration > 0.0 {
                body["progress"] = serde_json::json!(params.current_time / duration);
            }
        }
        let path = match &params.episode_id {
            Some(ep) => format!("/me/progress/{}/{}", params.item_id, ep),
            None => format!("/me/progress/{}", params.item_id),
        };
        self.client
            .patch(&path, &body)
            .await
            .map(|v| serde_json::to_string_pretty(&v).unwrap_or_default())
            .map_err(|e| e.to_string())
    }

    /// Create a bookmark at a specific playback position in an audiobook.
    /// Returns the updated list of bookmarks for the item.
    /// Enable with: --enable-tool create_bookmark
    #[tool]
    pub async fn create_bookmark(
        &self,
        Parameters(params): Parameters<CreateBookmarkParams>,
    ) -> Result<String, String> {
        let body = serde_json::json!({
            "time": params.time,
            "title": params.title,
        });
        self.client
            .post(&format!("/me/item/{}/bookmark", params.item_id), &body)
            .await
            .map(|v| serde_json::to_string_pretty(&v).unwrap_or_default())
            .map_err(|e| e.to_string())
    }

    /// Delete a bookmark at a specific playback position.
    /// The `time` value must exactly match an existing bookmark (use `get_item` to list bookmarks).
    /// Returns the updated list of bookmarks.
    /// Enable with: --enable-tool delete_bookmark
    #[tool]
    pub async fn delete_bookmark(
        &self,
        Parameters(params): Parameters<DeleteBookmarkParams>,
    ) -> Result<String, String> {
        self.client
            .delete(&format!(
                "/me/item/{}/bookmark/{}",
                params.item_id, params.time
            ))
            .await
            .map(|v| serde_json::to_string_pretty(&v).unwrap_or_default())
            .map_err(|e| e.to_string())
    }
}

#[tool_handler]
impl ServerHandler for AbsServer {}
