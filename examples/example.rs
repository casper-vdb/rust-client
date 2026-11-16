use casper_client::{
    BatchUpdateRequest, CasperClient, ClusterInfo, CompactionPatch, CreateCollectionRequest, HNSWIndexConfig, IndexPolicyPatch, IndexSpec, IndexState,
    IndexStorageSpec, ScoresRequest, SearchRequest, SnapshotPatch, UpsertOperation, VectorComponents,
};
use std::time::{Duration, Instant};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Resolve host and port from environment (with sane defaults).
    let host = std::env::var("CASPER_HOST").unwrap_or_else(|_| "http://127.0.0.1".to_string());
    let http_port: u16 = std::env::var("CASPER_HTTP_PORT").ok().and_then(|v| v.parse().ok()).unwrap_or(7222);

    let mut client = CasperClient::new(&host, http_port)?;
    // Only needed when the server has API keys configured.
    if let Ok(api_key) = std::env::var("CASPER_API_KEY") {
        client = client.with_api_key(api_key);
    }

    println!("Casper Vector Database Client Example");

    // Which node is this: its cluster role and Raft state, and one config value.
    match client.get_cluster().await? {
        ClusterInfo::Standalone => println!("Node runs standalone"),
        ClusterInfo::Cluster {
            node_id,
            raft_state,
            leader_id,
            ..
        } => println!("Node {} is a cluster {} (leader: {:?})", node_id, raft_state, leader_id),
    }
    let config = client.get_config().await?;
    println!("Storage dir: {}", config.storage.storage_dir);

    // 1. Create a collection with the schema and policy of its index.
    println!("\nCreating collection...");
    let request = CreateCollectionRequest {
        dim: 128,
        max_size: 10_000,
        index: IndexSpec {
            hnsw: HNSWIndexConfig {
                metric: "inner-product".to_string(),
                quantization: "f32".to_string(),
                m: 16,
                m0: 32,
                ef_construction: 200,
            },
            normalization: Some(true),
            // Optional: every key left out takes the server's default.
            storage: Some(IndexStorageSpec {
                snapshot: Some(SnapshotPatch {
                    changes: Some(1_000),
                    wal_ops: Some(2_000),
                    ..Default::default()
                }),
                ..Default::default()
            }),
        },
    };
    let id = client.create_collection("example_collection", request).await?;
    println!("Collection 'example_collection' created: {}", id);

    // 2. Upsert vectors.
    println!("\nUpserting vectors...");
    let upserts: Vec<UpsertOperation> = (1..=10)
        .map(|i| UpsertOperation {
            id: i,
            vector: generate_random_vector(128, i as f32),
        })
        .collect();
    client
        .batch_update("example_collection", BatchUpdateRequest { upserts, delete: vec![] })
        .await?;
    println!("Upserted 10 vectors");

    // 3. Upserts and deletes in one call.
    println!("\nBatch update (upsert + delete)...");
    let batch_request = BatchUpdateRequest {
        upserts: vec![UpsertOperation {
            id: 11,
            vector: generate_random_vector(128, 11.0),
        }],
        delete: vec![10],
    };
    client.batch_update("example_collection", batch_request).await?;
    println!("Upserted vector 11, deleted vector 10");

    // 4. Start indexing. The server returns 202 immediately and builds the
    //    index in the background; wait for `index.state` to become `ready`.
    //    `not_started` after a start means the build failed.
    println!("\nStarting indexing...");
    client.start_indexing("example_collection").await?;
    println!("Waiting for index build to finish...");
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let info = client.get_collection("example_collection").await?;
        match info.index.state {
            IndexState::Ready => break,
            IndexState::NotStarted => return Err("index build failed".into()),
            IndexState::Building if Instant::now() >= deadline => return Err("index build timed out".into()),
            IndexState::Building => tokio::time::sleep(Duration::from_millis(200)).await,
        }
    }
    println!("Index ready");

    // 5. Change the index policy, then snapshot and compact on demand where
    //    this node runs each mechanism (a trigger with it off is a 409).
    //    `max_chain_depth: Some(None)` turns the depth rule off; `None` would
    //    leave it as it is.
    println!("\nUpdating index policy...");
    let patch = IndexPolicyPatch {
        compaction: Some(CompactionPatch {
            max_chain_depth: Some(None),
            max_tail_ratio: Some(0.5),
            ..Default::default()
        }),
        ..Default::default()
    };
    client.update_index_policy("example_collection", patch).await?;
    println!("Policy updated");
    if config.storage.index.snapshot.enabled {
        client.trigger_snapshot("example_collection").await?;
        println!("Snapshot triggered");
    }
    if config.storage.index.compaction.enabled {
        client.trigger_compaction("example_collection").await?;
        println!("Compaction triggered");
    }

    // 6. Search.
    println!("\nSearching for similar vectors...");
    let query_vector = generate_random_vector(128, 1.0);
    let results = client.search("example_collection", 5, SearchRequest { vector: query_vector.clone() }).await?;
    println!("Found {} similar vectors:", results.len());
    for (i, result) in results.iter().enumerate() {
        println!("  {}. ID: {}, Score: {:.4}", i + 1, result.id, result.score);
    }

    // 7. Score the query against chosen ids; the deleted id 10 is left out.
    println!("\nScoring ids 1, 10 and 11...");
    let scores = client
        .scores(
            "example_collection",
            ScoresRequest {
                vector: query_vector,
                ids: vec![1, 10, 11],
            },
        )
        .await?;
    for result in &scores {
        println!("  ID: {}, Score: {:.4}", result.id, result.score);
    }

    // 8. Get a specific vector, as the index stores it.
    println!("\nGetting vector by ID...");
    match client.get_vector("example_collection", 1).await? {
        Some(VectorComponents::F32(vector)) => println!("Vector 1 retrieved: {} f32 components", vector.len()),
        Some(VectorComponents::I8(vector)) => println!("Vector 1 retrieved: {} i8 components", vector.len()),
        None => println!("Vector 1 not found"),
    }

    // 9. Get collection info.
    println!("\nGetting collection information...");
    let collection_info = client.get_collection("example_collection").await?;
    println!("  - Name: {}", collection_info.name);
    println!("  - Dimension: {}", collection_info.dimension);
    println!("  - Has index: {}", collection_info.has_index);
    println!("  - Max size: {}", collection_info.max_size);
    println!("  - Size: {}", collection_info.size);
    println!("  - Status: {}", collection_info.status);
    println!("  - Replicated: {}", collection_info.replicated);
    let wal = collection_info.wal;
    println!(
        "  - WAL: {} written, {} applied, {} snapshotted",
        wal.write_count, wal.applied_count, wal.snapshotted_count
    );
    let index = collection_info.index;
    let hnsw = index.hnsw;
    println!("  - Index: HNSW, {:?}", index.state);
    println!("    - Metric: {}", hnsw.metric);
    println!("    - Quantization: {}", hnsw.quantization);
    println!("    - M: {}", hnsw.m);
    println!("    - M0: {}", hnsw.m0);
    println!("    - Ef construction: {}", hnsw.ef_construction);
    println!("    - Normalization: {}", index.normalization);
    let storage = index.storage;
    println!("    - Range size: {} bytes", storage.range_size_bytes);
    println!(
        "    - Snapshot every {} changes / {} WAL ops",
        storage.snapshot.changes, storage.snapshot.wal_ops
    );
    println!("    - Max chain depth: {:?}", storage.compaction.max_chain_depth);

    // 10. List collections.
    println!("\nListing collections...");
    let collections = client.list_collections().await?;
    println!("Found {} collections:", collections.collections.len());
    for collection in collections.collections {
        println!("  - {}", collection.name);
    }

    // 11. Delete the collection (its index goes with it).
    println!("\nDeleting collection...");
    client.delete_collection("example_collection").await?;
    println!("Collection deleted");

    println!("\nExample completed successfully!");
    Ok(())
}

fn generate_random_vector(dim: usize, seed: f32) -> Vec<f32> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut vector = Vec::with_capacity(dim);
    for i in 0..dim {
        let mut hasher = DefaultHasher::new();
        (seed * 1000.0 + i as f32).to_bits().hash(&mut hasher);
        let hash = hasher.finish();
        let value = (hash as f32 / u64::MAX as f32) * 2.0 - 1.0;
        vector.push(value);
    }

    let norm: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in &mut vector {
            *value /= norm;
        }
    }

    vector
}
