use serde::{Deserialize, Deserializer, Serialize};

/// Body of `POST /collection/{name}/search`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchRequest {
    pub vector: Vec<f32>,
}

/// Body of `POST /collection/{name}/scores`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoresRequest {
    pub vector: Vec<f32>,
    /// The ids to score the query against, at most `search.max_scores_ids`.
    pub ids: Vec<u64>,
}

/// Search (or scores) result item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: u64,
    pub score: f32,
}

pub type SearchResponse = Vec<SearchResult>;

/// Body of `POST /collection/{name}`: the collection with the schema and
/// policy of its index. Creating a collection builds no index; see
/// `start_indexing`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCollectionRequest {
    pub dim: usize,
    pub max_size: u32,
    pub index: IndexSpec,
}

/// Response payload for `POST /collection/{name}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCollectionResponse {
    /// The new collection's uuid.
    pub id: String,
}

/// Information about an existing collection, as returned by
/// `GET /collections/{name}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionInfo {
    /// Collection status: "yellow" while an index is being built, else "green".
    pub status: String,
    /// The collection's uuid.
    pub id: String,
    pub name: String,
    pub dimension: usize,
    pub max_size: u32,
    /// Documents durably stored.
    pub size: u32,
    /// Documents visible to search. Lags `size` by the applier's backlog.
    pub applied_size: u32,
    pub has_index: bool,
    /// Whether the collection was created in cluster mode.
    pub replicated: bool,
    pub wal: WalCounters,
    /// The schema and policy the collection was created with, and where
    /// indexing is. Always present.
    pub index: IndexInfo,
    /// The counters the snapshot thresholds are compared against. Absent
    /// while no index is attached.
    #[serde(default)]
    pub snapshot: Option<SnapshotCounters>,
}

/// How many writes the WAL holds and how far the index has got through them,
/// each counted from the collection's creation. While no index is attached
/// the last two are 0.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalCounters {
    /// Writes durably in the WAL.
    pub write_count: u64,
    /// Writes applied to the in-memory index, so visible to search.
    pub applied_count: u64,
    /// Writes the index on disk covers.
    pub snapshotted_count: u64,
}

/// Progress towards the next snapshot: what `storage.snapshot.changes` and
/// `storage.snapshot.wal_ops` of the index are compared against.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotCounters {
    /// Internal ids whose payload changed since the last snapshot.
    pub changes: u64,
    /// WAL ops applied to the index since the last snapshot.
    pub wal_ops: u64,
}

/// `index.state` of [`CollectionInfo`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IndexState {
    /// Indexing has not been started — or, in standalone, a build failed.
    NotStarted,
    /// An index is being built here or, in a cluster, not fetched yet.
    Building,
    /// An index is attached; search answers.
    Ready,
}

/// Information about a collection's index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexInfo {
    pub state: IndexState,
    pub hnsw: HNSWIndexConfig,
    /// Whether vector normalization is applied on upsert.
    pub normalization: bool,
    /// How the index is kept on disk, as this node runs it.
    pub storage: IndexStorageInfo,
}

/// How an index is kept on disk, as reported in [`IndexInfo`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStorageInfo {
    /// Width of a payload range on disk. Fixed for the life of the index.
    pub range_size_bytes: u64,
    pub snapshot: SnapshotPolicy,
    pub compaction: CompactionPolicy,
    /// Whether this node snapshots the index: the node's flag and the
    /// policy's together.
    pub snapshot_active: bool,
    /// The same, for compaction.
    pub compaction_active: bool,
}

/// The snapshot policy of an index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotPolicy {
    pub enabled: bool,
    /// Changed ids since the last snapshot that trigger the next one.
    pub changes: u64,
    /// WAL ops since the last snapshot that trigger the next one.
    pub wal_ops: u64,
}

/// The compaction policy of an index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionPolicy {
    pub enabled: bool,
    /// `None` when the depth rule is off.
    pub max_chain_depth: Option<u32>,
    pub max_tail_ratio: f64,
}

/// One upsert of `BatchUpdateRequest`: the vector stored under `id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertOperation {
    pub id: u64,
    pub vector: Vec<f32>,
}

/// Body of `POST /collection/{name}/update`, the one write. Either list may
/// be empty, not both.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchUpdateRequest {
    pub upserts: Vec<UpsertOperation>,
    pub delete: Vec<u64>,
}

/// The `index` part of [`CreateCollectionRequest`]: the schema, which never
/// changes, and the policy, which `update_index_policy` can change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexSpec {
    pub hnsw: HNSWIndexConfig,
    /// Whether vectors are normalized on upsert. Server default: false.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalization: Option<bool>,
    /// How the index is kept on disk. Left out, every key takes the server's
    /// default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage: Option<IndexStorageSpec>,
}

/// The `index.storage` part of [`CreateCollectionRequest`]: every key optional.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IndexStorageSpec {
    /// Width of a payload range on disk, 128 MiB..8 GiB (default 128 MiB).
    /// Fixed for the life of the index.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range_size_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<SnapshotPatch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compaction: Option<CompactionPatch>,
}

/// `snapshot` as a request names it: every key optional, and only the named
/// keys change.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SnapshotPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// Changed ids since the last snapshot that trigger the next one (>= 1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changes: Option<u64>,
    /// WAL ops since the last snapshot that trigger the next one (>= changes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wal_ops: Option<u64>,
}

/// `compaction` as a request names it: every key optional, and only the named
/// keys change.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CompactionPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// Three states, because `null` is a value here: `None` leaves the rule as
    /// it is, `Some(None)` turns it off, `Some(Some(n))` sets it (n >= 3).
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "present")]
    pub max_chain_depth: Option<Option<u32>>,
    /// In (0, 1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tail_ratio: Option<f64>,
}

/// A key that is there, `null` included. `default` covers the key being absent.
fn present<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Option<u32>>, D::Error> {
    Option::<u32>::deserialize(deserializer).map(Some)
}

/// The snapshot and compaction policy change sent by
/// `PATCH /collection/{name}/index`. `range_size_bytes` cannot be changed.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct IndexPolicyPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<SnapshotPatch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compaction: Option<CompactionPatch>,
}

/// Body of `PATCH /collection/{name}/index`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateIndexPolicyRequest {
    pub storage: IndexPolicyPatch,
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

/// Response payload for `GET /collections`: names only, the rest is
/// `GET /collections/{name}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionsListResponse {
    pub collections: Vec<CollectionDescription>,
}

/// One entry of [`CollectionsListResponse`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionDescription {
    pub name: String,
}

/// Response payload for `GET /collection/{name}/vector/{id}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetVectorResponse {
    pub id: u64,
    pub vector: VectorComponents,
}

/// A vector exactly as the index stores it, with no conversion: `F32` for an
/// `f32` index (normalized if the index normalizes), `I8` for an `i8` index,
/// whose components are the quantized values in `[-100, 100]`
/// (`round(x * 100)`), not dequantized.
///
/// Decoded by shape: the server prints an `f32` component with a fraction
/// (`0.0`, not `0`), so integers are `I8` and anything else is `F32`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum VectorComponents {
    I8(Vec<i8>),
    F32(Vec<f32>),
}

impl VectorComponents {
    /// Number of components.
    pub fn len(&self) -> usize {
        match self {
            VectorComponents::I8(components) => components.len(),
            VectorComponents::F32(components) => components.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// ============================================================================
//  Cluster — /cluster
// ============================================================================

/// One member of the Raft membership, with both addresses peers dial it at.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterMemberInfo {
    pub node_id: String,
    /// Where peers dial this member for consensus.
    pub raft_addr: String,
    /// Where peers dial it for data-WAL replication.
    pub replication_addr: String,
    pub voter: bool,
}

/// Response payload for `GET /cluster`. The variant is tagged by the
/// server with `"role": "<standalone|cluster>"` and the rest of the fields
/// are inlined.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "lowercase")]
pub enum ClusterInfo {
    /// A standalone node has no cluster identity: `{"role": "standalone"}`.
    Standalone,
    Cluster {
        node_id: String,
        /// One of "leader", "follower", "candidate", "learner", "shutdown".
        raft_state: String,
        current_term: u64,
        /// Absent while no leader is known.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        leader_id: Option<String>,
        voters: Vec<String>,
        learners: Vec<String>,
        /// Every member, sorted by node id.
        members: Vec<ClusterMemberInfo>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        last_log_index: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        last_applied_index: Option<u64>,
    },
}

/// Response payload for `DELETE /cluster/peer/{node_id}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemovePeerResponse {
    /// The node id that was removed.
    pub removed: String,
}

/// Body of `PUT /cluster/transfer-limit`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetTransferLimitRequest {
    pub mb_per_sec: u64,
}

// ============================================================================
//  Configuration — GET /config
// ============================================================================

/// Cluster role of a Casper node. Lowercased on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClusterRole {
    Standalone,
    Cluster,
}

/// Response payload for `GET /config` — the configuration the node started
/// with, grouped the way the config file is. Configuration is immutable after
/// startup. TLS key paths and credentials are never included.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigResponse {
    pub service: ServiceConfig,
    pub cluster: ClusterConfig,
    pub storage: StorageConfig,
    pub search: SearchConfig,
    pub tls: TlsConfig,
    pub auth: AuthConfig,
    pub metrics: MetricsConfig,
}

/// `service` of [`ConfigResponse`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub host: String,
    pub http_port: u16,
    /// The HTTP address published to peers; `None` means derived from `host`
    /// and `http_port`.
    pub uri: Option<String>,
    pub shutdown_timeout_sec: u64,
}

/// `cluster` of [`ConfigResponse`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterConfig {
    pub role: ClusterRole,
    pub bootstrap: Option<String>,
    pub write_ack_timeout_ms: u64,
    pub max_inflight_entries: usize,
    pub max_apply_lag_mb: u64,
    pub write_queue_mb: usize,
    pub raft: RaftConfig,
    pub replication: ReplicationConfig,
}

/// `cluster.raft` of [`ConfigResponse`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaftConfig {
    pub port: u16,
    pub enable_tls: bool,
    pub uri: Option<String>,
}

/// `cluster.replication` of [`ConfigResponse`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicationConfig {
    pub port: u16,
    pub uri: Option<String>,
    pub max_transfer_mb_per_sec: u64,
}

/// `storage` of [`ConfigResponse`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub storage_dir: String,
    pub min_free_disk_mb: u64,
    pub wal: WalConfig,
    pub index: StorageIndexConfig,
}

/// `storage.wal` of [`ConfigResponse`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalConfig {
    pub max_batch_size_mb: usize,
    pub log_tail_mb: u64,
    pub snapshot_every_mb: u64,
    pub snapshot_every_entries: u64,
}

/// `storage.index` of [`ConfigResponse`]: only the node's switches; the
/// thresholds are each index's own, in [`IndexStorageInfo`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageIndexConfig {
    pub snapshot: IndexSnapshotConfig,
    pub compaction: IndexCompactionConfig,
}

/// `storage.index.snapshot` of [`ConfigResponse`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexSnapshotConfig {
    pub enabled: bool,
}

/// `storage.index.compaction` of [`ConfigResponse`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexCompactionConfig {
    pub enabled: bool,
}

/// `search` of [`ConfigResponse`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchConfig {
    /// The largest `limit` a search takes.
    pub max_search_limit: usize,
    /// The most ids one scores request may name.
    pub max_scores_ids: usize,
}

/// `tls` of [`ConfigResponse`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    pub enable: bool,
    pub verify_https_client_certificate: bool,
    pub tls_cert_ttl: Option<u64>,
}

/// `auth` of [`ConfigResponse`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub jwt_rbac: bool,
}

/// `metrics` of [`ConfigResponse`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// Prefix of every metric name.
    pub prefix: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_chain_depth_has_three_states() {
        let absent = CompactionPatch::default();
        assert_eq!(serde_json::to_string(&absent).unwrap(), "{}");

        let off = CompactionPatch {
            max_chain_depth: Some(None),
            ..Default::default()
        };
        assert_eq!(serde_json::to_string(&off).unwrap(), r#"{"max_chain_depth":null}"#);

        let set = CompactionPatch {
            max_chain_depth: Some(Some(8)),
            ..Default::default()
        };
        assert_eq!(serde_json::to_string(&set).unwrap(), r#"{"max_chain_depth":8}"#);

        for patch in [absent, off, set] {
            let bytes = serde_json::to_vec(&patch).unwrap();
            assert_eq!(serde_json::from_slice::<CompactionPatch>(&bytes).unwrap(), patch);
        }
    }

    #[test]
    fn create_collection_request_omits_unset_keys() {
        let request = CreateCollectionRequest {
            dim: 4,
            max_size: 1_000_000,
            index: IndexSpec {
                hnsw: HNSWIndexConfig {
                    metric: "euclidean".to_string(),
                    quantization: "f32".to_string(),
                    m: 16,
                    m0: 32,
                    ef_construction: 200,
                },
                normalization: Some(true),
                storage: None,
            },
        };
        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "dim": 4,
                "max_size": 1000000,
                "index": {
                    "hnsw": { "metric": "euclidean", "quantization": "f32", "m": 16, "m0": 32, "ef_construction": 200 },
                    "normalization": true
                }
            })
        );
    }

    #[test]
    fn create_collection_request_carries_storage_under_index() {
        let request = CreateCollectionRequest {
            dim: 768,
            max_size: 1_000_000,
            index: IndexSpec {
                hnsw: HNSWIndexConfig {
                    metric: "inner-product".to_string(),
                    quantization: "f32".to_string(),
                    m: 16,
                    m0: 32,
                    ef_construction: 200,
                },
                normalization: None,
                storage: Some(IndexStorageSpec {
                    compaction: Some(CompactionPatch {
                        max_chain_depth: Some(None),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
            },
        };
        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["index"]["storage"], serde_json::json!({"compaction": {"max_chain_depth": null}}));
    }

    #[test]
    fn batch_update_request_names_upserts() {
        let request = BatchUpdateRequest {
            upserts: vec![UpsertOperation {
                id: u64::MAX,
                vector: vec![0.5],
            }],
            delete: vec![7],
        };
        assert_eq!(
            serde_json::to_value(&request).unwrap(),
            serde_json::json!({"upserts": [{"id": u64::MAX, "vector": [0.5]}], "delete": [7]})
        );
    }

    #[test]
    fn collection_info_from_api_md() {
        let body = r#"{
          "status": "green",
          "id": "d0d84675-4cb5-4663-9463-bb261848fc36",
          "name": "demo",
          "dimension": 96,
          "max_size": 10000000,
          "size": 9990000,
          "applied_size": 9990000,
          "has_index": true,
          "replicated": false,
          "wal": { "write_count": 52813, "applied_count": 52813, "snapshotted_count": 52813 },
          "index": {
            "state": "ready",
            "hnsw": {
              "metric": "inner-product",
              "quantization": "f32",
              "m": 16,
              "m0": 32,
              "ef_construction": 200
            },
            "normalization": true,
            "storage": {
              "range_size_bytes": 134217728,
              "snapshot": { "enabled": true, "changes": 100000, "wal_ops": 200000 },
              "compaction": { "enabled": true, "max_chain_depth": 8, "max_tail_ratio": 0.25 },
              "snapshot_active": true,
              "compaction_active": true
            }
          },
          "snapshot": { "changes": 0, "wal_ops": 0 }
        }"#;
        let info: CollectionInfo = serde_json::from_str(body).unwrap();
        assert_eq!(info.index.state, IndexState::Ready);
        assert_eq!(info.index.hnsw.metric, "inner-product");
        assert_eq!(info.index.storage.compaction.max_chain_depth, Some(8));
        assert_eq!(info.wal.snapshotted_count, 52813);
        assert!(info.snapshot.is_some());
    }

    #[test]
    fn collection_info_before_indexing() {
        let body = r#"{"status":"green","id":"d0d84675-4cb5-4663-9463-bb261848fc36","name":"demo","dimension":4,
            "max_size":100,"size":3,"applied_size":0,"has_index":false,"replicated":false,
            "wal":{"write_count":1,"applied_count":0,"snapshotted_count":0},
            "index":{"state":"not_started",
              "hnsw":{"metric":"cosine","quantization":"i8","m":16,"m0":32,"ef_construction":200},
              "normalization":false,
              "storage":{"range_size_bytes":134217728,
                "snapshot":{"enabled":true,"changes":10,"wal_ops":20},
                "compaction":{"enabled":true,"max_chain_depth":null,"max_tail_ratio":0.25},
                "snapshot_active":true,"compaction_active":true}}}"#;
        let info: CollectionInfo = serde_json::from_str(body).unwrap();
        assert_eq!(info.index.state, IndexState::NotStarted);
        assert_eq!(info.index.storage.compaction.max_chain_depth, None);
        assert!(info.snapshot.is_none());
    }

    #[test]
    fn get_vector_decodes_f32_and_i8_from_api_md() {
        let f32_body = r#"{"id": 42, "vector": [0.1, 0.2, 0.3, 0.4]}"#;
        let response: GetVectorResponse = serde_json::from_str(f32_body).unwrap();
        assert_eq!(response.id, 42);
        assert_eq!(response.vector, VectorComponents::F32(vec![0.1, 0.2, 0.3, 0.4]));

        let i8_body = r#"{"id": 42, "vector": [10, 20, 30, 40]}"#;
        let response: GetVectorResponse = serde_json::from_str(i8_body).unwrap();
        assert_eq!(response.vector, VectorComponents::I8(vec![10, 20, 30, 40]));

        let i8_bounds = r#"{"id": 1, "vector": [-100, 0, 100]}"#;
        let response: GetVectorResponse = serde_json::from_str(i8_bounds).unwrap();
        assert_eq!(response.vector, VectorComponents::I8(vec![-100, 0, 100]));

        // An f32 index prints a whole component with a fraction.
        let whole_f32 = r#"{"id": 1, "vector": [0.0, 1.0, 0.0]}"#;
        let response: GetVectorResponse = serde_json::from_str(whole_f32).unwrap();
        assert_eq!(response.vector, VectorComponents::F32(vec![0.0, 1.0, 0.0]));
        assert_eq!(response.vector.len(), 3);
    }

    #[test]
    fn cluster_info_standalone_from_api_md() {
        let body = r#"{"role": "standalone"}"#;
        match serde_json::from_str::<ClusterInfo>(body).unwrap() {
            ClusterInfo::Standalone => {}
            other => panic!("expected Standalone, got {:?}", other),
        }
    }

    #[test]
    fn cluster_info_cluster_from_api_md() {
        // docs/api.md's example, with the member and the two indexes the code
        // also sends.
        let body = r#"{
          "role": "cluster",
          "node_id": "ea1ae526-cd80-47f6-9ea8-346909637655",
          "raft_state": "leader",
          "current_term": 3,
          "leader_id": "ea1ae526-cd80-47f6-9ea8-346909637655",
          "voters": ["09a656b1-c95e-46ea-87b2-721b7e6f8ebf", "ea1ae526-cd80-47f6-9ea8-346909637655"],
          "learners": [],
          "members": [
            {
              "node_id": "ea1ae526-cd80-47f6-9ea8-346909637655",
              "raft_addr": "127.0.0.1:7224",
              "replication_addr": "127.0.0.1:7223",
              "voter": true
            }
          ],
          "last_log_index": 4,
          "last_applied_index": 4
        }"#;
        match serde_json::from_str::<ClusterInfo>(body).unwrap() {
            ClusterInfo::Cluster {
                raft_state,
                leader_id,
                voters,
                members,
                last_applied_index,
                ..
            } => {
                assert_eq!(raft_state, "leader");
                assert_eq!(leader_id.as_deref(), Some("ea1ae526-cd80-47f6-9ea8-346909637655"));
                assert_eq!(voters.len(), 2);
                assert_eq!(members[0].raft_addr, "127.0.0.1:7224");
                assert_eq!(members[0].replication_addr, "127.0.0.1:7223");
                assert_eq!(last_applied_index, Some(4));
            }
            other => panic!("expected Cluster, got {:?}", other),
        }

        let as_documented = r#"{"role":"cluster","node_id":"ea1ae526-cd80-47f6-9ea8-346909637655","raft_state":"leader","current_term":3,
            "leader_id":"ea1ae526-cd80-47f6-9ea8-346909637655","voters":[],"learners":[],"members":[]}"#;
        assert!(serde_json::from_str::<ClusterInfo>(as_documented).is_ok());

        let leaderless = r#"{"role":"cluster","node_id":"a","raft_state":"candidate","current_term":2,"voters":[],"learners":[],"members":[]}"#;
        assert!(matches!(
            serde_json::from_str::<ClusterInfo>(leaderless).unwrap(),
            ClusterInfo::Cluster {
                leader_id: None,
                last_log_index: None,
                ..
            }
        ));
    }

    #[test]
    fn config_from_api_md() {
        // docs/api.md's example, with `service.shutdown_timeout_sec`, which the
        // code also sends.
        let body = r#"{
          "service": { "host": "127.0.0.1", "http_port": 7222, "uri": null, "shutdown_timeout_sec": 25 },
          "cluster": {
            "role": "standalone",
            "bootstrap": null,
            "write_ack_timeout_ms": 5000,
            "max_inflight_entries": 16,
            "max_apply_lag_mb": 1024,
            "write_queue_mb": 128,
            "raft": { "port": 7224, "enable_tls": false, "uri": null },
            "replication": { "port": 7223, "uri": null, "max_transfer_mb_per_sec": 50 }
          },
          "storage": {
            "storage_dir": "./storage",
            "min_free_disk_mb": 8192,
            "wal": { "max_batch_size_mb": 1, "log_tail_mb": 1024, "snapshot_every_mb": 256, "snapshot_every_entries": 5000 },
            "index": {
              "snapshot": { "enabled": true },
              "compaction": { "enabled": true }
            }
          },
          "search": { "max_search_limit": 100000, "max_scores_ids": 100000 },
          "tls": { "enable": false, "verify_https_client_certificate": false, "tls_cert_ttl": null },
          "auth": { "jwt_rbac": false },
          "metrics": { "prefix": "casper_" }
        }"#;
        let config: ConfigResponse = serde_json::from_str(body).unwrap();
        assert_eq!(config.service.http_port, 7222);
        assert_eq!(config.cluster.role, ClusterRole::Standalone);
        assert_eq!(config.cluster.replication.max_transfer_mb_per_sec, 50);
        assert_eq!(config.storage.wal.snapshot_every_entries, 5000);
        assert!(config.storage.index.compaction.enabled);
        assert_eq!(config.tls.tls_cert_ttl, None);
        assert_eq!(config.metrics.prefix, "casper_");
        assert_eq!(config.search.max_search_limit, 100000);
        assert_eq!(config.search.max_scores_ids, 100000);
        assert_eq!(config.service.shutdown_timeout_sec, 25);
    }
}
