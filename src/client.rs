use crate::error::{CasperError, Result};
use crate::models::*;
use reqwest::{Client, Method, RequestBuilder, Response};
use std::time::Duration;
use url::Url;

/// Casper vector database client (HTTP).
#[derive(Debug, Clone)]
pub struct CasperClient {
    client: Client,
    base_url: Url,
    timeout: Duration,
    api_key: Option<String>,
}

/// How long `remove_peer` waits: longer than the server's own 30 s wait for
/// the removal to commit, so that its 504 reaches the caller rather than a
/// client timeout.
const REMOVE_PEER_TIMEOUT: Duration = Duration::from_secs(60);

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
        if base_url.cannot_be_a_base() {
            return Err(CasperError::Url(url::ParseError::RelativeUrlWithCannotBeABaseBase));
        }
        let client = Client::builder().timeout(timeout).build()?;

        Ok(Self {
            client,
            base_url,
            timeout,
            api_key: None,
        })
    }

    /// Send `api-key: <key>` with every request, for a server with API keys
    /// configured. `/health` and `/readyz` do not need it.
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Base URL the client targets.
    pub fn base_url(&self) -> &str {
        self.base_url.as_str()
    }

    // ---- Collection lifecycle ----------------------------------------------

    /// `GET /collections`
    pub async fn list_collections(&self) -> Result<CollectionsListResponse> {
        let url = self.endpoint(&["collections"]);
        let response = self.request(Method::GET, url).send().await?;
        self.handle_response(response).await
    }

    /// `GET /collections/{name}`
    pub async fn get_collection(&self, collection_name: &str) -> Result<CollectionInfo> {
        let url = self.endpoint(&["collections", collection_name]);
        let response = self.request(Method::GET, url).send().await?;
        self.handle_response(response).await
    }

    /// `POST /collection/{name}` — create a collection with the schema and
    /// policy of its index, and return its uuid. No index is built until
    /// [`start_indexing`](Self::start_indexing).
    pub async fn create_collection(&self, collection_name: &str, request: CreateCollectionRequest) -> Result<String> {
        let url = self.endpoint(&["collection", collection_name]);
        let response = self.request(Method::POST, url).json(&request).send().await?;
        let created: CreateCollectionResponse = self.handle_response(response).await?;
        Ok(created.id)
    }

    /// `DELETE /collection/{name}`
    pub async fn delete_collection(&self, collection_name: &str) -> Result<()> {
        let url = self.endpoint(&["collection", collection_name]);
        let response = self.request(Method::DELETE, url).send().await?;
        self.handle_empty_response(response).await
    }

    // ---- Vector operations -------------------------------------------------

    /// `POST /collection/{name}/update` — upserts and deletes in one call, the
    /// one write.
    pub async fn batch_update(&self, collection_name: &str, request: BatchUpdateRequest) -> Result<()> {
        let url = self.endpoint(&["collection", collection_name, "update"]);
        let response = self.request(Method::POST, url).json(&request).send().await?;
        self.handle_empty_response(response).await
    }

    /// `GET /collection/{name}/vector/{id}` — the vector as the index stores
    /// it (see [`VectorComponents`]). `None` when the index holds no such id;
    /// a missing collection is an error.
    pub async fn get_vector(&self, collection_name: &str, id: u64) -> Result<Option<VectorComponents>> {
        let id = id.to_string();
        let url = self.endpoint(&["collection", collection_name, "vector", &id]);
        let response = self.request(Method::GET, url).send().await?;

        if response.status() == 404 {
            let text = response.text().await?;
            if is_vector_not_found(&text) {
                return Ok(None);
            }
            return Err(parse_error_response(404, &text, None));
        }

        let vector_response: GetVectorResponse = self.handle_response(response).await?;
        Ok(Some(vector_response.vector))
    }

    // ---- Index lifecycle ---------------------------------------------------

    /// `POST /collection/{name}/index/start` — start indexing on the schema
    /// the collection was created with. The build is asynchronous: the server
    /// returns 202 and `index.state` of [`get_collection`](Self::get_collection)
    /// goes `building`, then `ready` — or back to `not_started` if a
    /// standalone build failed. A repeated call is a no-op.
    pub async fn start_indexing(&self, collection_name: &str) -> Result<()> {
        let url = self.endpoint(&["collection", collection_name, "index", "start"]);
        let response = self.request(Method::POST, url).send().await?;
        self.handle_empty_response(response).await
    }

    /// `PATCH /collection/{name}/index` — change the snapshot and compaction
    /// policy of the collection's index, before indexing starts as well. Only
    /// the keys the patch names change.
    pub async fn update_index_policy(&self, collection_name: &str, patch: IndexPolicyPatch) -> Result<()> {
        let url = self.endpoint(&["collection", collection_name, "index"]);
        let response = self
            .request(Method::PATCH, url)
            .json(&UpdateIndexPolicyRequest { storage: patch })
            .send()
            .await?;
        self.handle_empty_response(response).await
    }

    /// `POST /collection/{name}/index/snapshot/trigger` — snapshot the index
    /// on this node now, regardless of thresholds. Returns once accepted (202).
    pub async fn trigger_snapshot(&self, collection_name: &str) -> Result<()> {
        let url = self.endpoint(&["collection", collection_name, "index", "snapshot", "trigger"]);
        let response = self.request(Method::POST, url).send().await?;
        self.handle_empty_response(response).await
    }

    /// `POST /collection/{name}/index/compaction/trigger` — start a fold pass
    /// on this node now; it folds only the ranges the policy's rules hold for.
    /// Returns once accepted (202).
    pub async fn trigger_compaction(&self, collection_name: &str) -> Result<()> {
        let url = self.endpoint(&["collection", collection_name, "index", "compaction", "trigger"]);
        let response = self.request(Method::POST, url).send().await?;
        self.handle_empty_response(response).await
    }

    // ---- Search ------------------------------------------------------------

    /// `POST /collection/{name}/search?limit=..&output=bin` — uses the binary
    /// response format for compactness and decodes it client-side.
    pub async fn search(&self, collection_name: &str, limit: usize, request: SearchRequest) -> Result<SearchResponse> {
        self.search_with_ef(collection_name, limit, None, request).await
    }

    /// Same as [`search`](Self::search) but lets the caller override `ef_search` on the
    /// server side (HNSW candidate list size).
    pub async fn search_with_ef(&self, collection_name: &str, limit: usize, ef_search: Option<usize>, request: SearchRequest) -> Result<SearchResponse> {
        let url = self.endpoint(&["collection", collection_name, "search"]);

        let mut query: Vec<(&str, String)> = vec![("limit", limit.to_string()), ("output", "bin".to_string())];
        if let Some(ef) = ef_search {
            query.push(("ef_search", ef.to_string()));
        }

        let response = self.request(Method::POST, url).query(&query).json(&request).send().await?;

        let status = response.status();
        if !status.is_success() {
            return Err(self.error_from_response(response).await);
        }

        let bytes = response.bytes().await?;
        decode_scored(bytes.as_ref())
    }

    /// `POST /collection/{name}/scores?output=bin` — score the query against
    /// the vector each of `request.ids` holds, as a search scores a match. The
    /// result is in the order of `ids`; an id the index does not hold is left
    /// out, an id given twice is answered twice.
    pub async fn scores(&self, collection_name: &str, request: ScoresRequest) -> Result<SearchResponse> {
        let url = self.endpoint(&["collection", collection_name, "scores"]);
        let response = self.request(Method::POST, url).query(&[("output", "bin")]).json(&request).send().await?;

        if !response.status().is_success() {
            return Err(self.error_from_response(response).await);
        }

        let bytes = response.bytes().await?;
        decode_scored(bytes.as_ref())
    }

    // ---- Cluster, configuration & metrics -----------------------------------

    /// `GET /cluster` — node role and Raft state. The variant of
    /// [`ClusterInfo`] reflects whether this node is `Standalone` or a
    /// `Cluster` member.
    pub async fn get_cluster(&self) -> Result<ClusterInfo> {
        let url = self.endpoint(&["cluster"]);
        let response = self.request(Method::GET, url).send().await?;
        self.handle_response(response).await
    }

    /// `DELETE /cluster/peer/{node_id}` — remove a node from the cluster
    /// membership. Must be sent to the leader; a follower answers with
    /// [`CasperError::NotLeader`].
    pub async fn remove_peer(&self, node_id: &str) -> Result<RemovePeerResponse> {
        let url = self.endpoint(&["cluster", "peer", node_id]);
        let response = self.request(Method::DELETE, url).timeout(self.timeout.max(REMOVE_PEER_TIMEOUT)).send().await?;
        self.handle_response(response).await
    }

    /// `PUT /cluster/transfer-limit` — set the rate limit of index and WAL
    /// transfers sent by this node. Held in memory only: a restart returns to
    /// the configured value.
    pub async fn set_transfer_limit(&self, mb_per_sec: u64) -> Result<()> {
        let url = self.endpoint(&["cluster", "transfer-limit"]);
        let response = self.request(Method::PUT, url).json(&SetTransferLimitRequest { mb_per_sec }).send().await?;
        self.handle_empty_response(response).await
    }

    /// `GET /config` — the configuration the node started with. The config is
    /// immutable after startup; restart the server to change a value.
    pub async fn get_config(&self) -> Result<ConfigResponse> {
        let url = self.endpoint(&["config"]);
        let response = self.request(Method::GET, url).send().await?;
        self.handle_response(response).await
    }

    /// `GET /metrics` — Prometheus metrics in the text exposition format.
    pub async fn get_metrics(&self) -> Result<String> {
        let url = self.endpoint(&["metrics"]);
        let response = self.request(Method::GET, url).send().await?;
        if !response.status().is_success() {
            return Err(self.error_from_response(response).await);
        }
        Ok(response.text().await?)
    }

    // ---- Health ------------------------------------------------------------

    /// `GET /health` — returns `Ok(())` when the server process is up.
    /// Liveness only; see [`ready`](Self::ready) for readiness.
    pub async fn health(&self) -> Result<()> {
        let url = self.endpoint(&["health"]);
        let response = self.request(Method::GET, url).send().await?;
        self.handle_empty_response(response).await
    }

    /// `GET /readyz?quorum=..` — whether this node should get reads.
    /// `Ok(true)` on 200, `Ok(false)` on 503 (joining, leaderless or
    /// stopping). With `quorum` the node must also count for quorum, which
    /// is what a rolling restart waits for.
    pub async fn ready(&self, quorum: bool) -> Result<bool> {
        let url = self.endpoint(&["readyz"]);
        let response = self.request(Method::GET, url).query(&[("quorum", quorum)]).send().await?;
        if response.status() == 503 {
            return Ok(false);
        }
        self.handle_empty_response(response).await?;
        Ok(true)
    }

    // ---- Internal helpers --------------------------------------------------

    /// The URL of `segments` under the base URL, each segment percent-encoded,
    /// so a name holding `/`, `?` or `#` stays one segment.
    fn endpoint(&self, segments: &[&str]) -> Url {
        let mut url = self.base_url.clone();
        url.path_segments_mut()
            .expect("the constructor refuses a base URL that cannot be a base")
            .pop_if_empty()
            .extend(segments);
        url
    }

    fn request(&self, method: Method, url: Url) -> RequestBuilder {
        let request = self.client.request(method, url);
        match &self.api_key {
            Some(api_key) => request.header("api-key", api_key),
            None => request,
        }
    }

    async fn handle_response<T>(&self, response: Response) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        if !response.status().is_success() {
            return Err(self.error_from_response(response).await);
        }

        let text = response.text().await?;
        serde_json::from_str(&text).map_err(|e| CasperError::InvalidResponse(format!("Failed to parse response: {} - {}", e, text)))
    }

    async fn handle_empty_response(&self, response: Response) -> Result<()> {
        if response.status().is_success() {
            Ok(())
        } else {
            Err(self.error_from_response(response).await)
        }
    }

    async fn error_from_response(&self, response: Response) -> CasperError {
        let status = response.status().as_u16();
        let retry_after = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<u64>().ok())
            .map(Duration::from_secs);
        match response.text().await {
            Ok(text) => parse_error_response(status, &text, retry_after),
            Err(e) => CasperError::Http(e),
        }
    }
}

/// The binary answer of search and scores: `[u32 LE count]` then `count`
/// records of `(u64 LE id, f32 LE score)`, 12 bytes each.
fn decode_scored(buf: &[u8]) -> Result<SearchResponse> {
    const RECORD_LEN: usize = 12;

    let Some((count_bytes, records)) = buf.split_first_chunk::<4>() else {
        return Err(CasperError::InvalidResponse("binary search response too short (missing count)".to_string()));
    };
    let count = u32::from_le_bytes(*count_bytes) as usize;

    let expected_len = count.checked_mul(RECORD_LEN);
    if expected_len.is_none_or(|len| records.len() < len) {
        return Err(CasperError::InvalidResponse(format!(
            "binary search response truncated: {} records announced, {} bytes of records received",
            count,
            records.len()
        )));
    }

    let results = records
        .chunks_exact(RECORD_LEN)
        .take(count)
        .map(|record| {
            let (id_bytes, score_bytes) = record.split_at(8);
            let id = u64::from_le_bytes(id_bytes.try_into().expect("8 bytes"));
            let score = f32::from_le_bytes(score_bytes.try_into().expect("4 bytes"));
            SearchResult { id, score }
        })
        .collect();

    Ok(results)
}

/// The one 404 that means "no such vector": JSON `{"error": "Vector not
/// found", "id": ..}`. Any other 404, a missing collection among them, is an
/// error.
fn is_vector_not_found(text: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(text)
        .ok()
        .and_then(|body| body.get("error").and_then(|v| v.as_str()).map(|message| message == "Vector not found"))
        .unwrap_or(false)
}

fn parse_error_response(status: u16, text: &str, retry_after: Option<Duration>) -> CasperError {
    // Only the not-leader 405 has a JSON body; every other 405 is plain text.
    if let Ok(error_json) = serde_json::from_str::<serde_json::Value>(text)
        && let Some(message) = error_json.get("error").and_then(|v| v.as_str())
    {
        if status == 405 {
            let leader_addr = error_json.get("leader_addr").and_then(|v| v.as_str()).map(str::to_string);
            return CasperError::NotLeader {
                message: message.to_string(),
                leader_addr,
            };
        }
        return CasperError::from_status(status, message.to_string(), retry_after);
    }
    CasperError::from_status(status, text.to_string(), retry_after)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = CasperClient::new("http://localhost", 7222).unwrap();
        assert_eq!(client.base_url(), "http://localhost:7222/");
    }

    #[test]
    fn a_path_segment_is_escaped() {
        let client = CasperClient::new("http://localhost", 7222).unwrap();
        let url = client.endpoint(&["collection", "a/b?c#d", "search"]);
        assert_eq!(url.as_str(), "http://localhost:7222/collection/a%2Fb%3Fc%23d/search");
    }

    #[test]
    fn a_405_with_json_names_the_leader() {
        let body = r#"{"error":"Method not allowed: mutating operations require the Raft leader","leader_addr":"http://10.0.0.7:7222"}"#;
        match parse_error_response(405, body, None) {
            CasperError::NotLeader { leader_addr, .. } => assert_eq!(leader_addr.as_deref(), Some("http://10.0.0.7:7222")),
            other => panic!("expected NotLeader, got {:?}", other),
        }

        let leaderless = r#"{"error":"Method not allowed: mutating operations require the Raft leader"}"#;
        assert!(matches!(
            parse_error_response(405, leaderless, None),
            CasperError::NotLeader { leader_addr: None, .. }
        ));

        assert!(matches!(
            parse_error_response(405, "Method not allowed: x", None),
            CasperError::OperationNotAllowed(_)
        ));
    }

    fn encode_scored(records: &[(u64, f32)]) -> Vec<u8> {
        let mut buf = (records.len() as u32).to_le_bytes().to_vec();
        for (id, score) in records {
            buf.extend_from_slice(&id.to_le_bytes());
            buf.extend_from_slice(&score.to_le_bytes());
        }
        buf
    }

    #[test]
    fn binary_records_are_twelve_bytes() {
        let buf = encode_scored(&[(u64::MAX, 0.5), (7, -1.25)]);
        assert_eq!(buf.len(), 4 + 2 * 12);
        let results = decode_scored(&buf).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!((results[0].id, results[0].score), (u64::MAX, 0.5));
        assert_eq!((results[1].id, results[1].score), (7, -1.25));

        assert!(decode_scored(&encode_scored(&[])).unwrap().is_empty());
    }

    #[test]
    fn a_short_binary_answer_is_refused() {
        assert!(matches!(decode_scored(&[1, 0]), Err(CasperError::InvalidResponse(_))));

        let mut truncated = encode_scored(&[(1, 0.5)]);
        truncated.pop();
        assert!(matches!(decode_scored(&truncated), Err(CasperError::InvalidResponse(_))));

        // A count whose record length does not fit is refused, not overflowed.
        assert!(matches!(decode_scored(&u32::MAX.to_le_bytes()), Err(CasperError::InvalidResponse(_))));
    }

    #[test]
    fn only_the_json_vector_not_found_is_absent() {
        assert!(is_vector_not_found(r#"{"error":"Vector not found","id":42}"#));
        assert!(!is_vector_not_found("Resource not found: collection 'demo' not found"));
        assert!(!is_vector_not_found(r#"{"error":"collection not found"}"#));
    }

    #[test]
    fn retry_after_is_surfaced() {
        let error = parse_error_response(429, "Too many requests: queue full", Some(Duration::from_secs(3)));
        assert!(matches!(error, CasperError::TooManyRequests { .. }));
        assert_eq!(error.retry_after(), Some(Duration::from_secs(3)));

        let error = parse_error_response(503, "Service unavailable: joining", Some(Duration::from_secs(5)));
        assert_eq!(error.retry_after(), Some(Duration::from_secs(5)));
    }
}
