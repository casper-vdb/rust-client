use serde::{Deserialize, Serialize};

/// Vector insertion request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsertRequest {
    pub id: u32,
    pub vector: Vec<f32>,
}

/// Vector insertion body (for JSON payload)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsertVectorBody {
    pub vector: Vec<f32>,
}

/// Vector deletion request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteRequest {
    pub id: u32,
}

/// Search request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchRequest {
    pub vector: Vec<f32>,
    pub limit: Option<usize>,
}

/// Search vector body (for JSON payload)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchVectorBody {
    pub vector: Vec<f32>,
}

/// Search result item (tuple format: [id, score])
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: u32,
    pub score: f32,
}

/// Search response (array of [id, score] tuples)
pub type SearchResponse = Vec<SearchResult>;

/// Collection creation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCollectionRequest {
    pub dim: usize,
    pub max_size: u32,
}

/// Collection information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionInfo {
    pub name: String,
    pub dimension: usize,
    pub mutable: bool,
    pub has_index: bool,
    pub max_size: u32,
    /// Current number of vectors in the collection
    pub size: usize,
    pub index: Option<IndexInfo>,
}

/// Index information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexInfo {
    /// HNSW index configuration (if present)
    pub hnsw: Option<HNSWIndexConfig>,
    /// Whether normalization is applied for this index
    pub normalization: bool,
}

/// Batch insert operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchInsertOperation {
    pub id: u32,
    pub vector: Vec<f32>,
}

/// Batch update request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchUpdateRequest {
    pub insert: Vec<BatchInsertOperation>,
    pub delete: Vec<u32>,
}

/// Index creation request for HNSW
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateHNSWIndexRequest {
    /// HNSW index configuration
    pub hnsw: HNSWIndexConfig,
    /// Whether to apply vector normalization
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalization: Option<bool>,
}

/// HNSW index configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HNSWIndexConfig {
    /// Distance metric, e.g. "inner-product"
    pub metric: String,
    /// Quantization type, e.g. "f32" or "pq8"
    pub quantization: String,
    /// Number of bi-directional links created for every new element
    pub m: usize,
    /// Number of outgoing connections in the zero layer
    pub m0: usize,
    /// Controls index search speed/build speed tradeoff
    pub ef_construction: usize,
    /// Optional PQ name when using product quantization
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pq_name: Option<String>,
}

/// Collections list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionsListResponse {
    pub collections: Vec<CollectionInfo>,
}

/// Get vector response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetVectorResponse {
    pub id: u32,
    pub vector: Vec<f32>,
}

/// Matrix information (from /matrix APIs)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatrixInfo {
    pub name: String,
    pub dim: usize,
    pub len: usize,
    pub enabled: bool,
}

/// Result of gRPC matrix upload
#[derive(Debug, Clone)]
pub struct UploadMatrixResult {
    pub success: bool,
    pub message: String,
    pub total_vectors: u32,
    pub total_chunks: u32,
}

/// Create PQ request (for /pq/{name})
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePqRequest {
    pub dim: usize,
    pub codebooks: Vec<String>,
}

/// PQ info (for /pq APIs)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PqInfo {
    pub name: String,
    pub dim: usize,
    pub codebooks: Vec<String>,
    pub enabled: bool,
}
