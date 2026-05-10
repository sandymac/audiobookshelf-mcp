// Copyright (c) 2026 Sandy McArthur, Jr.
// SPDX-License-Identifier: MIT

use std::collections::HashSet;
use std::sync::Arc;

use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_handler, tool_router, ServerHandler};
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::abs::AbsClient;

/// Registry of all tools: (name, group, default_enabled).
/// Used for --list-tools output and to build the enabled set at startup.
pub const TOOL_REGISTRY: &[(&str, &str, bool)] = &[
    ("list_libraries", "default", true),
    ("search_library", "default", true),
    ("get_library_items", "default", true),
    ("get_item", "default", true),
    ("get_in_progress", "default", true),
    ("get_listening_stats", "default", true),
    ("get_recent_sessions", "default", true),
    ("get_metadata_object", "metadata", true),
    ("find_items_missing_metadata", "metadata", true),
    ("update_progress", "progress", false),
    ("create_bookmark", "progress", false),
    ("delete_bookmark", "progress", false),
    ("quick_match_item", "metadata", false),
    ("batch_quick_match_items", "metadata", false),
    ("batch_update_metadata", "metadata", false),
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
        Self {
            client,
            tool_router,
        }
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

#[derive(Deserialize, schemars::JsonSchema)]
pub struct QuickMatchItemParams {
    /// Library item ID to quick-match.
    pub item_id: String,
    /// Metadata provider to use, such as "audible", "google", or "openlibrary".
    #[serde(default)]
    pub provider: Option<String>,
    /// Title override to guide the match.
    #[serde(default)]
    pub title: Option<String>,
    /// Author override to guide the match.
    #[serde(default)]
    pub author: Option<String>,
    /// ISBN override to guide the match.
    #[serde(default)]
    pub isbn: Option<String>,
    /// ASIN override to guide the match.
    #[serde(default)]
    pub asin: Option<String>,
    /// Replace the existing cover when true.
    #[serde(default)]
    pub override_cover: Option<bool>,
    /// Replace existing metadata details when true.
    #[serde(default)]
    pub override_details: Option<bool>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct BatchQuickMatchItemsParams {
    /// Library item IDs to quick-match.
    pub library_item_ids: Vec<String>,
    /// Metadata provider to use, such as "audible", "google", or "openlibrary".
    #[serde(default)]
    pub provider: Option<String>,
    /// Replace existing covers when true.
    #[serde(default)]
    pub override_cover: Option<bool>,
    /// Replace existing metadata details when true.
    #[serde(default)]
    pub override_details: Option<bool>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct MetadataUpdate {
    /// Library item ID to update.
    pub item_id: String,
    /// Audiobookshelf media metadata object to apply.
    pub metadata: Value,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct BatchUpdateMetadataParams {
    /// Metadata updates to apply.
    pub updates: Vec<MetadataUpdate>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct GetMetadataObjectParams {
    /// Library item ID. Obtain from `search_library` or `get_library_items`.
    pub item_id: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct FindItemsMissingMetadataParams {
    /// Library ID to scan. Obtain IDs from `list_libraries`.
    pub library_id: String,
    /// Page number, 0-based (default: 0).
    #[serde(default)]
    pub page: Option<u32>,
    /// Items per page, max 100 (default: 100).
    #[serde(default)]
    pub limit: Option<u32>,
}

// ── Payload / summary helpers ────────────────────────────────────────────────

fn build_quick_match_payload(params: &QuickMatchItemParams) -> Value {
    let mut body = Map::new();
    insert_optional_string(&mut body, "provider", &params.provider);
    insert_optional_string(&mut body, "title", &params.title);
    insert_optional_string(&mut body, "author", &params.author);
    insert_optional_string(&mut body, "isbn", &params.isbn);
    insert_optional_string(&mut body, "asin", &params.asin);
    insert_optional_bool(&mut body, "overrideCover", params.override_cover);
    insert_optional_bool(&mut body, "overrideDetails", params.override_details);
    Value::Object(body)
}

fn build_batch_quick_match_payload(params: &BatchQuickMatchItemsParams) -> Value {
    let mut body = Map::new();
    body.insert(
        "libraryItemIds".into(),
        Value::Array(
            params
                .library_item_ids
                .iter()
                .map(|id| Value::String(id.clone()))
                .collect(),
        ),
    );
    let mut options = Map::new();
    insert_optional_string(&mut options, "provider", &params.provider);
    insert_optional_bool(&mut options, "overrideCover", params.override_cover);
    insert_optional_bool(&mut options, "overrideDetails", params.override_details);
    if !options.is_empty() {
        body.insert("options".into(), Value::Object(options));
    }
    Value::Object(body)
}

fn build_batch_update_metadata_payload(params: &BatchUpdateMetadataParams) -> Value {
    Value::Array(
        params
            .updates
            .iter()
            .map(|update| {
                serde_json::json!({
                    "id": update.item_id,
                    "mediaPayload": {
                        "metadata": update.metadata
                    }
                })
            })
            .collect(),
    )
}

fn summarize_items_missing_metadata(
    library_id: &str,
    page: u32,
    limit: u32,
    response: &Value,
) -> Value {
    let items = extract_items(response);
    let mut missing_items = Vec::new();

    for item in &items {
        let missing = missing_metadata_fields(item);
        if missing.is_empty() {
            continue;
        }
        missing_items.push(serde_json::json!({
            "id": item.get("id").and_then(Value::as_str).unwrap_or_default(),
            "title": item_title(item),
            "missing_fields": missing,
        }));
    }

    serde_json::json!({
        "library_id": library_id,
        "page": page,
        "limit": limit,
        "scanned": items.len(),
        "missing_count": missing_items.len(),
        "items": missing_items,
    })
}

fn insert_optional_string(body: &mut Map<String, Value>, key: &str, value: &Option<String>) {
    if let Some(value) = value {
        body.insert(key.into(), Value::String(value.clone()));
    }
}

fn insert_optional_bool(body: &mut Map<String, Value>, key: &str, value: Option<bool>) {
    if let Some(value) = value {
        body.insert(key.into(), Value::Bool(value));
    }
}

fn extract_items(response: &Value) -> Vec<&Value> {
    if let Some(items) = response.as_array() {
        return items.iter().collect();
    }
    for key in ["results", "items", "libraryItems"] {
        if let Some(items) = response.get(key).and_then(Value::as_array) {
            return items.iter().collect();
        }
    }
    Vec::new()
}

fn missing_metadata_fields(item: &Value) -> Vec<&'static str> {
    let mut missing = Vec::new();
    let metadata = item
        .get("media")
        .and_then(|media| media.get("metadata"))
        .or_else(|| item.get("metadata"));

    for field in [
        "title",
        "authors",
        "narrators",
        "description",
        "publishedYear",
        "isbn",
        "asin",
        "genres",
        "series",
    ] {
        if metadata
            .and_then(|metadata| metadata.get(field))
            .map(is_metadata_value_present)
            != Some(true)
        {
            missing.push(field);
        }
    }

    if item
        .get("coverPath")
        .or_else(|| item.get("media").and_then(|media| media.get("coverPath")))
        .or_else(|| metadata.and_then(|metadata| metadata.get("coverPath")))
        .map(is_metadata_value_present)
        != Some(true)
    {
        missing.push("coverPath");
    }

    missing
}

fn is_metadata_value_present(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::String(s) => !s.trim().is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
        _ => true,
    }
}

fn item_title(item: &Value) -> String {
    item.get("media")
        .and_then(|media| media.get("metadata"))
        .and_then(|metadata| metadata.get("title"))
        .and_then(Value::as_str)
        .or_else(|| {
            item.get("metadata")
                .and_then(|metadata| metadata.get("title"))
                .and_then(Value::as_str)
        })
        .or_else(|| item.get("title").and_then(Value::as_str))
        .or_else(|| item.get("name").and_then(Value::as_str))
        .unwrap_or("")
        .to_string()
}

// ── Tools ─────────────────────────────────────────────────────────────────────

#[tool_router]
impl AbsServer {
    /// List all libraries on the Audiobookshelf server.
    /// Returns each library's ID, name, media type (book or podcast), and folder paths.
    /// Use the library IDs with other tools to browse or search content.
    #[tool(
        title = "List Libraries",
        annotations(read_only_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
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
    #[tool(
        title = "Search Library",
        annotations(read_only_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
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
    #[tool(
        title = "Get Library Items",
        annotations(read_only_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
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
    #[tool(
        title = "Get Item",
        annotations(read_only_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
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

    /// Get the raw metadata object for a library item.
    /// Use `search_library` or `get_library_items` to find item IDs.
    #[tool(
        title = "Get Metadata Object",
        annotations(read_only_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
    pub async fn get_metadata_object(
        &self,
        Parameters(params): Parameters<GetMetadataObjectParams>,
    ) -> Result<String, String> {
        self.client
            .get(&format!("/items/{}/metadata-object", params.item_id))
            .await
            .map(|v| serde_json::to_string_pretty(&v).unwrap_or_default())
            .map_err(|e| e.to_string())
    }

    /// Get all audiobooks and podcast episodes currently in progress for the authenticated user.
    /// Shows each item's current position, progress percentage, and time remaining.
    #[tool(
        title = "Get In Progress",
        annotations(read_only_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
    pub async fn get_in_progress(&self) -> Result<String, String> {
        self.client
            .get("/me/items-in-progress")
            .await
            .map(|v| serde_json::to_string_pretty(&v).unwrap_or_default())
            .map_err(|e| e.to_string())
    }

    /// Get listening statistics for the authenticated user: total time, time per day of week,
    /// daily breakdown, and most-listened items.
    #[tool(
        title = "Get Listening Stats",
        annotations(read_only_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
    pub async fn get_listening_stats(&self) -> Result<String, String> {
        self.client
            .get("/me/listening-stats")
            .await
            .map(|v| serde_json::to_string_pretty(&v).unwrap_or_default())
            .map_err(|e| e.to_string())
    }

    /// Get recent playback sessions for the authenticated user, paginated.
    /// Each session shows the item played, start/end time, and minutes listened.
    #[tool(
        title = "Get Recent Sessions",
        annotations(read_only_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
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

    /// Find library items that appear to be missing common metadata fields.
    /// Scans one minified page and returns item IDs, titles, and missing field names.
    #[tool(
        title = "Find Items Missing Metadata",
        annotations(read_only_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
    pub async fn find_items_missing_metadata(
        &self,
        Parameters(params): Parameters<FindItemsMissingMetadataParams>,
    ) -> Result<String, String> {
        let limit = params.limit.unwrap_or(100).min(100);
        let page = params.page.unwrap_or(0);
        self.client
            .get_with_params(
                &format!("/libraries/{}/items", params.library_id),
                &[
                    ("limit", limit.to_string()),
                    ("page", page.to_string()),
                    ("minified", "1".into()),
                ],
            )
            .await
            .map(|v| {
                serde_json::to_string(&summarize_items_missing_metadata(
                    &params.library_id,
                    page,
                    limit,
                    &v,
                ))
                .unwrap_or_default()
            })
            .map_err(|e| e.to_string())
    }

    /// Update listening progress for an audiobook or podcast episode.
    /// Use to record playback position, mark an item finished, or reset progress.
    /// For podcasts, provide episode_id. Enable with: --enable-tool update_progress
    #[tool(
        title = "Update Progress",
        annotations(
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
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
    #[tool(
        title = "Create Bookmark",
        annotations(destructive_hint = false, open_world_hint = false)
    )]
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
    #[tool(
        title = "Delete Bookmark",
        annotations(
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
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

    /// Quick-match metadata for one library item using Audiobookshelf's metadata providers.
    /// Mutates the item metadata. Enable with: --enable-tool quick_match_item
    #[tool(
        title = "Quick Match Item",
        annotations(destructive_hint = false, open_world_hint = false)
    )]
    pub async fn quick_match_item(
        &self,
        Parameters(params): Parameters<QuickMatchItemParams>,
    ) -> Result<String, String> {
        let body = build_quick_match_payload(&params);
        self.client
            .post(&format!("/items/{}/match", params.item_id), &body)
            .await
            .map(|v| serde_json::to_string_pretty(&v).unwrap_or_default())
            .map_err(|e| e.to_string())
    }

    /// Quick-match metadata for multiple library items.
    /// Mutates item metadata. Enable with: --enable-tool batch_quick_match_items
    #[tool(
        title = "Batch Quick Match Items",
        annotations(destructive_hint = false, open_world_hint = false)
    )]
    pub async fn batch_quick_match_items(
        &self,
        Parameters(params): Parameters<BatchQuickMatchItemsParams>,
    ) -> Result<String, String> {
        let body = build_batch_quick_match_payload(&params);
        self.client
            .post("/items/batch/quickmatch", &body)
            .await
            .map(|v| serde_json::to_string_pretty(&v).unwrap_or_default())
            .map_err(|e| e.to_string())
    }

    /// Batch-update metadata for multiple library items.
    /// Mutates item metadata. Enable with: --enable-tool batch_update_metadata
    #[tool(
        title = "Batch Update Metadata",
        annotations(destructive_hint = false, open_world_hint = false)
    )]
    pub async fn batch_update_metadata(
        &self,
        Parameters(params): Parameters<BatchUpdateMetadataParams>,
    ) -> Result<String, String> {
        let body = build_batch_update_metadata_payload(&params);
        self.client
            .post("/items/batch/update", &body)
            .await
            .map(|v| serde_json::to_string_pretty(&v).unwrap_or_default())
            .map_err(|e| e.to_string())
    }
}

#[tool_handler]
impl ServerHandler for AbsServer {}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn metadata_mutation_tools_are_disabled_by_default() {
        for name in [
            "quick_match_item",
            "batch_quick_match_items",
            "batch_update_metadata",
        ] {
            let (_, _, enabled) = TOOL_REGISTRY
                .iter()
                .find(|(tool, _, _)| *tool == name)
                .unwrap_or_else(|| panic!("{name} should be registered"));
            assert!(!enabled, "{name} should be disabled by default");
        }
    }

    #[test]
    fn metadata_read_tools_are_enabled_by_default() {
        for name in ["get_metadata_object", "find_items_missing_metadata"] {
            let (_, _, enabled) = TOOL_REGISTRY
                .iter()
                .find(|(tool, _, _)| *tool == name)
                .unwrap_or_else(|| panic!("{name} should be registered"));
            assert!(enabled, "{name} should be enabled by default");
        }
    }

    #[test]
    fn builds_quick_match_payload_with_optional_fields() {
        let payload = build_quick_match_payload(&QuickMatchItemParams {
            item_id: "item-1".into(),
            provider: Some("audible".into()),
            title: Some("The Left Hand of Darkness".into()),
            author: Some("Ursula K. Le Guin".into()),
            isbn: Some("9780441478125".into()),
            asin: Some("B000FC1BN8".into()),
            override_cover: Some(true),
            override_details: Some(false),
        });

        assert_eq!(
            payload,
            json!({
                "provider": "audible",
                "title": "The Left Hand of Darkness",
                "author": "Ursula K. Le Guin",
                "isbn": "9780441478125",
                "asin": "B000FC1BN8",
                "overrideCover": true,
                "overrideDetails": false
            })
        );
    }

    #[test]
    fn builds_batch_quick_match_payload_with_camel_case_ids() {
        let payload = build_batch_quick_match_payload(&BatchQuickMatchItemsParams {
            library_item_ids: vec!["item-1".into(), "item-2".into()],
            provider: Some("google".into()),
            override_cover: Some(false),
            override_details: Some(true),
        });

        assert_eq!(
            payload,
            json!({
                "libraryItemIds": ["item-1", "item-2"],
                "options": {
                    "provider": "google",
                    "overrideCover": false,
                    "overrideDetails": true
                }
            })
        );
    }

    #[test]
    fn builds_batch_update_metadata_payload_entries() {
        let payload = build_batch_update_metadata_payload(&BatchUpdateMetadataParams {
            updates: vec![MetadataUpdate {
                item_id: "item-1".into(),
                metadata: json!({
                    "title": "A Wizard of Earthsea",
                    "publishedYear": "1968",
                    "genres": ["Fantasy"]
                }),
            }],
        });

        assert_eq!(
            payload,
            json!([
                {
                    "id": "item-1",
                    "mediaPayload": {
                        "metadata": {
                            "title": "A Wizard of Earthsea",
                            "publishedYear": "1968",
                            "genres": ["Fantasy"]
                        }
                    }
                }
            ])
        );
    }

    #[test]
    fn summarizes_items_missing_metadata() {
        let response = json!({
            "results": [
                {
                    "id": "complete",
                    "media": {
                        "coverPath": "/covers/complete.jpg",
                        "metadata": {
                            "title": "Complete Book",
                            "authors": [{"name": "Author"}],
                            "narrators": ["Narrator"],
                            "description": "A book.",
                            "publishedYear": "2020",
                            "isbn": "123",
                            "asin": "B123",
                            "genres": ["Fiction"],
                            "series": [{"name": "Series"}]
                        }
                    }
                },
                {
                    "id": "missing",
                    "media": {
                        "metadata": {
                            "title": "Sparse Book",
                            "authors": [],
                            "description": "",
                            "genres": []
                        }
                    }
                }
            ]
        });

        let summary = summarize_items_missing_metadata("lib-1", 2, 50, &response);

        assert_eq!(
            summary,
            json!({
                "library_id": "lib-1",
                "page": 2,
                "limit": 50,
                "scanned": 2,
                "missing_count": 1,
                "items": [{
                    "id": "missing",
                    "title": "Sparse Book",
                    "missing_fields": [
                        "authors",
                        "narrators",
                        "description",
                        "publishedYear",
                        "isbn",
                        "asin",
                        "genres",
                        "series",
                        "coverPath"
                    ]
                }]
            })
        );
    }
}
