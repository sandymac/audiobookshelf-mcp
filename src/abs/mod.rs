use anyhow::Context;
use reqwest::{header, Client, StatusCode};
use serde_json::Value;

pub struct AbsClient {
    client: Client,
    base_url: String,
}

impl AbsClient {
    pub fn new(server_url: &str, api_token: &str) -> anyhow::Result<Self> {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            header::HeaderValue::from_str(&format!("Bearer {api_token}"))
                .context("invalid API token value")?,
        );
        let client = Client::builder()
            .default_headers(headers)
            .build()
            .context("failed to build HTTP client")?;
        let base_url = server_url.trim_end_matches('/').to_string();
        Ok(Self { client, base_url })
    }

    /// Verify the API token by calling /api/me and log the authenticated username.
    pub async fn test_connection(&self) -> anyhow::Result<()> {
        let url = format!("{}/api/me", self.base_url);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .context("failed to reach server")?;
        let status = resp.status();
        if !status.is_success() {
            anyhow::bail!(
                "connection test failed: HTTP {} — check --server-url and --api-token",
                status
            );
        }
        let user: Value = resp.json().await.context("unexpected response from /api/me")?;
        tracing::info!(
            username = user["username"].as_str().unwrap_or("unknown"),
            server = %self.base_url,
            "connected to Audiobookshelf"
        );
        Ok(())
    }

    // ── HTTP helpers ──────────────────────────────────────────────────────────

    pub async fn get(&self, path: &str) -> anyhow::Result<Value> {
        let url = format!("{}/api{}", self.base_url, path);
        let resp = self.client.get(&url).send().await?;
        self.parse_response(resp).await
    }

    pub async fn get_with_params(&self, path: &str, params: &[(&str, String)]) -> anyhow::Result<Value> {
        let url = format!("{}/api{}", self.base_url, path);
        let resp = self.client.get(&url).query(params).send().await?;
        self.parse_response(resp).await
    }

    pub async fn patch(&self, path: &str, body: &Value) -> anyhow::Result<Value> {
        let url = format!("{}/api{}", self.base_url, path);
        let resp = self.client.patch(&url).json(body).send().await?;
        self.parse_response(resp).await
    }

    pub async fn post(&self, path: &str, body: &Value) -> anyhow::Result<Value> {
        let url = format!("{}/api{}", self.base_url, path);
        let resp = self.client.post(&url).json(body).send().await?;
        self.parse_response(resp).await
    }

    pub async fn delete(&self, path: &str) -> anyhow::Result<Value> {
        let url = format!("{}/api{}", self.base_url, path);
        let resp = self.client.delete(&url).send().await?;
        self.parse_response(resp).await
    }

    async fn parse_response(&self, resp: reqwest::Response) -> anyhow::Result<Value> {
        let status = resp.status();
        if !status.is_success() {
            let hint = error_hint(status);
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("HTTP {}: {}{}", status, body.trim(), hint);
        }
        let bytes = resp.bytes().await?;
        if bytes.is_empty() {
            return Ok(serde_json::json!({ "success": true }));
        }
        serde_json::from_slice(&bytes).context("failed to parse JSON response")
    }
}

fn error_hint(status: StatusCode) -> &'static str {
    match status.as_u16() {
        401 | 403 => " [Hint: API token is invalid or expired — generate a new one in Settings → Users → API Keys]",
        404 => " [Hint: resource not found — check the item/library ID is correct]",
        429 => " [Hint: rate limited — wait before retrying]",
        503 => " [Hint: Audiobookshelf is temporarily unavailable — try again later]",
        _ => "",
    }
}
