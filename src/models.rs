use serde::{Deserialize, Serialize};

/// A single (id, vector) pair used in upsert/batch-insert operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorPoint {
    pub id: u32,
    pub vector: Vec<f32>,
}

/// Body of `PUT /collection/{name}/vectors`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertVectorsRequest {
    pub vectors: Vec<VectorPoint>,
}

/// Body of `POST /collection/{name}/vectors/delete`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteVectorsRequest {
    pub vectors: Vec<u32>,
}

/// Body of `POST /collection/{name}/search`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchRequest {
    pub vector: Vec<f32>,
}

/// Search result item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: u32,
    pub score: f32,
}

pub type SearchResponse = Vec<SearchResult>;

/// Query parameters for `POST /collection/{name}/create`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCollectionRequest {
    pub dim: usize,
    pub max_size: u32,
}

/// Information about an existing collection, as returned by
/// `GET /collection/{name}` and `GET /collections`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionInfo {
    pub name: String,
    pub dimension: usize,
    pub mutable: bool,
    pub has_index: bool,
    pub max_size: u32,
    /// Current number of vectors in the collection.
    pub size: u32,
    /// Collection status (e.g. "green", "yellow").
    pub status: String,
    #[serde(default)]
    pub index: Option<IndexInfo>,
    /// WAL head — highest durably-fsynced data WAL seq + 1. Available since
    /// Casper v0.1.0; defaults to 0 against older servers.
    #[serde(default)]
    pub current_seq: u64,
    /// Highest WAL seq applied to the in-memory HNSW. `current_seq -
    /// applied_seq` is the applier backlog. Casper v0.1.0+.
    #[serde(default)]
    pub applied_seq: u64,
    /// WAL seq covered by the on-disk snapshot, if any. Absent until a
    /// snapshot exists. Casper v0.1.0+.
    #[serde(default)]
    pub index_seq: Option<u64>,
    /// Per-collection catalog head (catalog WAL seq + 1). Casper v0.1.0+.
    #[serde(default)]
    pub catalog_seq: u64,
}

/// Information about a collection's index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexInfo {
    /// HNSW configuration when an HNSW index is present.
    #[serde(default)]
    pub hnsw: Option<HNSWIndexConfig>,
    /// Whether vector normalization is applied on insert/update.
    pub normalization: bool,
}

/// Batch operation item used by `BatchUpdateRequest`. Kept as a distinct
/// type for clarity at call sites, even though it has the same shape as
/// `VectorPoint`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchInsertOperation {
    pub id: u32,
    pub vector: Vec<f32>,
}

/// Body of `POST /collection/{name}/update`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchUpdateRequest {
    pub insert: Vec<BatchInsertOperation>,
    pub delete: Vec<u32>,
}

/// Body of `POST /collection/{name}/index`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateHNSWIndexRequest {
    pub hnsw: HNSWIndexConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalization: Option<bool>,
}

/// HNSW index configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HNSWIndexConfig {
    /// Distance metric: "euclidean", "l2sq", "cosine", or "inner-product".
    pub metric: String,
    /// Quantization: "f32" or "i8".
    pub quantization: String,
    /// Bi-directional links per node on upper layers.
    pub m: usize,
    /// Bi-directional links per node on the bottom layer (level 0).
    pub m0: usize,
    /// Candidate list size during index build.
    pub ef_construction: usize,
}

/// Response payload for `GET /collections`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionsListResponse {
    pub collections: Vec<CollectionInfo>,
}

/// Response payload for `GET /collection/{name}/vector/{id}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetVectorResponse {
    pub id: u32,
    pub vector: Vec<f32>,
}

// ============================================================================
//  Cluster — GET /cluster
// ============================================================================

/// Per-collection progress reported by a master for one of its local
/// collections.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterCollectionInfo {
    pub name: String,
    pub catalog_seq: u64,
    pub current_seq: u64,
    pub applied_seq: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index_seq: Option<u64>,
}

/// Per-collection progress reported by a replica for one of the collections
/// it tracks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicaCollectionInfo {
    pub name: String,
    pub applied_catalog_seq: u64,
    pub applied_data_seq: u64,
}

/// One connected replica's last-known cursors, as reported by ACKs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicaCursorInfo {
    pub replica_id: String,
    pub applied_global_catalog_seq: u64,
    pub collections: Vec<ReplicaCollectionInfo>,
}

/// Response payload for `GET /cluster`. The variant is tagged by the
/// server with `"role": "<standalone|master|replica>"` and the rest of
/// the fields are inlined.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "lowercase")]
pub enum ClusterInfo {
    Standalone {
        node_id: String,
    },
    Master {
        node_id: String,
        global_catalog_seq: u64,
        collections: Vec<MasterCollectionInfo>,
        replicas: Vec<ReplicaCursorInfo>,
    },
    Replica {
        node_id: String,
        applied_global_catalog_seq: u64,
        master_addr: String,
        collections: Vec<ReplicaCollectionInfo>,
    },
}

// ============================================================================
//  Configuration — GET /config
// ============================================================================

/// Cluster role of a Casper node. Lowercased on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClusterRole {
    Standalone,
    Master,
    Replica,
}

/// Response payload for `GET /config` — the runtime `AppConfig` the server
/// process started with. Configuration is immutable after startup.
///
/// New fields may be added by future Casper versions; `#[serde(default)]`
/// keeps deserialization tolerant against newer servers that send unknown
/// fields (those are ignored by default) and older servers that omit
/// fields added in newer Casper releases.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub host: String,
    pub http_port: u16,
    pub storage_dir: String,
    pub cluster_role: ClusterRole,
    #[serde(default)]
    pub master_addr: Option<String>,
    pub grpc_port: u16,
    pub snapshot_every_writes: u64,
    pub snapshot_every_seconds: u64,
    pub snapshot_enabled: bool,
    pub sync_replicas: usize,
    pub write_ack_timeout_ms: u64,
}
