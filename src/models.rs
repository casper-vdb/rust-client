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
