use crate::error::{CasperError, Result};
use crate::models::*;
use reqwest::Client;
use std::time::Duration;
use url::Url;

/// Casper vector database client (HTTP).
#[derive(Debug, Clone)]
pub struct CasperClient {
    client: Client,
    base_url: Url,
}

impl CasperClient {
    /// Create a new client.
    ///
    /// - `host`: hostname with scheme (e.g. "http://127.0.0.1")
    /// - `http_port`: HTTP API port (Casper default is 7222)
    pub fn new(host: &str, http_port: u16) -> Result<Self> {
        Self::with_timeout(host, http_port, Duration::from_secs(30))
    }

    /// Create a new client with a custom request timeout.
    pub fn with_timeout(host: &str, http_port: u16, timeout: Duration) -> Result<Self> {
        let base_url_str = format!("{}:{}", host, http_port);
        let base_url = Url::parse(&base_url_str)?;
        let client = Client::builder().timeout(timeout).build()?;

        Ok(Self { client, base_url })
    }

    /// Base URL the client targets.
    pub fn base_url(&self) -> &str {
        self.base_url.as_str()
    }

    // ---- Collection lifecycle ----------------------------------------------

    /// `GET /collections`
    pub async fn list_collections(&self) -> Result<CollectionsListResponse> {
        let url = self.base_url.join("collections")?;
        let response = self.client.get(url).send().await?;
        self.handle_response(response).await
    }

    /// `GET /collection/{name}`
    pub async fn get_collection(&self, collection_name: &str) -> Result<CollectionInfo> {
        let url = self.base_url.join(&format!("collection/{}", collection_name))?;
        let response = self.client.get(url).send().await?;
        self.handle_response(response).await
    }

    /// `POST /collection/{name}?dim=..&max_size=..`
    pub async fn create_collection(&self, collection_name: &str, request: CreateCollectionRequest) -> Result<()> {
        let url = self.base_url.join(&format!("collection/{}", collection_name))?;
        let response = self.client.post(url).query(&request).send().await?;
        self.handle_empty_response(response).await
    }

    /// `DELETE /collection/{name}`
    pub async fn delete_collection(&self, collection_name: &str) -> Result<()> {
        let url = self.base_url.join(&format!("collection/{}", collection_name))?;
        let response = self.client.delete(url).send().await?;
        self.handle_empty_response(response).await
    }

    // ---- Vector operations -------------------------------------------------

    /// `PUT /collection/{name}/vectors` — upsert one or more vectors.
    pub async fn upsert_vectors(&self, collection_name: &str, vectors: Vec<VectorPoint>) -> Result<()> {
        let url = self.base_url.join(&format!("collection/{}/vectors", collection_name))?;
        let response = self
            .client
            .put(url)
            .header("Content-Type", "application/json")
            .json(&UpsertVectorsRequest { vectors })
            .send()
            .await?;
        self.handle_empty_response(response).await
    }

    /// `POST /collection/{name}/vectors/delete` — delete vectors by id.
    pub async fn delete_vectors(&self, collection_name: &str, ids: Vec<u32>) -> Result<()> {
        let url = self.base_url.join(&format!("collection/{}/vectors/delete", collection_name))?;
        let response = self
            .client
            .post(url)
            .header("Content-Type", "application/json")
            .json(&DeleteVectorsRequest { vectors: ids })
            .send()
            .await?;
        self.handle_empty_response(response).await
    }

    /// `POST /collection/{name}/update` — mixed insert + delete in one call.
    pub async fn batch_update(&self, collection_name: &str, request: BatchUpdateRequest) -> Result<()> {
        let url = self.base_url.join(&format!("collection/{}/update", collection_name))?;
        let response = self.client.post(url).header("Content-Type", "application/json").json(&request).send().await?;
        self.handle_empty_response(response).await
    }

    /// `GET /collection/{name}/vector/{id}` — returns `None` on 404.
    pub async fn get_vector(&self, collection_name: &str, id: u32) -> Result<Option<Vec<f32>>> {
        let url = self.base_url.join(&format!("collection/{}/vector/{}", collection_name, id))?;
        let response = self.client.get(url).send().await?;

        if response.status() == 404 {
            return Ok(None);
        }

        let vector_response: GetVectorResponse = self.handle_response(response).await?;
        Ok(Some(vector_response.vector))
    }

    // ---- Mute / unmute -----------------------------------------------------

    /// `POST /collection/{name}/mute`
    pub async fn mute(&self, collection_name: &str) -> Result<()> {
        let url = self.base_url.join(&format!("collection/{}/mute", collection_name))?;
        let response = self.client.post(url).send().await?;
        self.handle_empty_response(response).await
    }

    /// `POST /collection/{name}/unmute`
    pub async fn unmute(&self, collection_name: &str) -> Result<()> {
        let url = self.base_url.join(&format!("collection/{}/unmute", collection_name))?;
        let response = self.client.post(url).send().await?;
        self.handle_empty_response(response).await
    }

    // ---- Index lifecycle ---------------------------------------------------

    /// `POST /collection/{name}/index` — index build is asynchronous; the
    /// server returns 202 Accepted as soon as the build has been scheduled.
    pub async fn create_hnsw_index(&self, collection_name: &str, request: CreateHNSWIndexRequest) -> Result<()> {
        let url = self.base_url.join(&format!("collection/{}/index", collection_name))?;
        let response = self.client.post(url).header("Content-Type", "application/json").json(&request).send().await?;
        self.handle_empty_response(response).await
    }

    /// `DELETE /collection/{name}/index`
    pub async fn delete_index(&self, collection_name: &str) -> Result<()> {
        let url = self.base_url.join(&format!("collection/{}/index", collection_name))?;
        let response = self.client.delete(url).send().await?;
        self.handle_empty_response(response).await
    }

    // ---- Search ------------------------------------------------------------

    /// `POST /collection/{name}/search?limit=..&output=bin` — uses the binary
    /// response format for compactness and decodes it client-side.
    pub async fn search(&self, collection_name: &str, limit: usize, request: SearchRequest) -> Result<SearchResponse> {
        self.search_with_ef(collection_name, limit, None, request).await
    }

    /// Same as [`search`] but lets the caller override `ef_search` on the
    /// server side (HNSW candidate list size).
    pub async fn search_with_ef(&self, collection_name: &str, limit: usize, ef_search: Option<usize>, request: SearchRequest) -> Result<SearchResponse> {
        let url = self.base_url.join(&format!("collection/{}/search", collection_name))?;

        let mut query: Vec<(&str, String)> = vec![("limit", limit.to_string()), ("output", "bin".to_string())];
        if let Some(ef) = ef_search {
            query.push(("ef_search", ef.to_string()));
        }

        let response = self
            .client
            .post(url)
            .query(&query)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await?;
            return Err(self.parse_error_response(status.as_u16(), &text));
        }

        let bytes = response.bytes().await?;
        let buf = bytes.as_ref();

        // Binary format: [u32 LE count] then `count` * (u32 LE id, f32 LE score).
        if buf.len() < 4 {
            return Err(CasperError::InvalidResponse("binary search response too short (missing count)".to_string()));
        }

        let mut offset = 0;
        let mut count_bytes = [0u8; 4];
        count_bytes.copy_from_slice(&buf[offset..offset + 4]);
        let count = u32::from_le_bytes(count_bytes) as usize;
        offset += 4;

        let expected_len = 4 + count * 8;
        if buf.len() < expected_len {
            return Err(CasperError::InvalidResponse(format!(
                "binary search response truncated: expected at least {} bytes, got {}",
                expected_len,
                buf.len()
            )));
        }

        let mut results = Vec::with_capacity(count);
        for _ in 0..count {
            let mut id_bytes = [0u8; 4];
            id_bytes.copy_from_slice(&buf[offset..offset + 4]);
            let id = u32::from_le_bytes(id_bytes);
            offset += 4;

            let mut score_bytes = [0u8; 4];
            score_bytes.copy_from_slice(&buf[offset..offset + 4]);
            let score = f32::from_le_bytes(score_bytes);
            offset += 4;

            results.push(SearchResult { id, score });
        }

        Ok(results)
    }

    // ---- Cluster & configuration -------------------------------------------

    /// `GET /cluster` — node role + replication progress. The variant of
    /// [`ClusterInfo`] reflects whether this node is `Standalone`, `Master`,
    /// or `Replica`.
    pub async fn get_cluster(&self) -> Result<ClusterInfo> {
        let url = self.base_url.join("cluster")?;
        let response = self.client.get(url).send().await?;
        self.handle_response(response).await
    }

    /// `GET /config` — runtime configuration the server started with. The
    /// config is immutable after startup; restart the server to change a
    /// value.
    pub async fn get_config(&self) -> Result<AppConfig> {
        let url = self.base_url.join("config")?;
        let response = self.client.get(url).send().await?;
        self.handle_response(response).await
    }

    /// `GET /health` — returns `Ok(())` when the server process is up.
    /// Suitable for Kubernetes liveness/readiness probes.
    pub async fn health(&self) -> Result<()> {
        let url = self.base_url.join("health")?;
        let response = self.client.get(url).send().await?;
        self.handle_empty_response(response).await
    }

    // ---- Internal helpers --------------------------------------------------

    async fn handle_response<T>(&self, response: reqwest::Response) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let status = response.status();
        let text = response.text().await?;

        if status.is_success() {
            serde_json::from_str(&text).map_err(|e| CasperError::InvalidResponse(format!("Failed to parse response: {} - {}", e, text)))
        } else {
            Err(self.parse_error_response(status.as_u16(), &text))
        }
    }

    async fn handle_empty_response(&self, response: reqwest::Response) -> Result<()> {
        let status = response.status();
        if status.is_success() {
            Ok(())
        } else {
            let text = response.text().await?;
            Err(self.parse_error_response(status.as_u16(), &text))
        }
    }

    fn parse_error_response(&self, status: u16, text: &str) -> CasperError {
        if let Ok(error_json) = serde_json::from_str::<serde_json::Value>(text) {
            if let Some(message) = error_json.get("error").and_then(|v| v.as_str()) {
                return CasperError::from_status(status, message.to_string());
            }
        }
        CasperError::from_status(status, text.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = CasperClient::new("http://localhost", 7222).unwrap();
        assert_eq!(client.base_url(), "http://localhost:7222/");
    }
}
