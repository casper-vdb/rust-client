use casper_client::{
    BatchInsertOperation, BatchUpdateRequest, CasperClient, CreateCollectionRequest, CreateHNSWIndexRequest, HNSWIndexConfig, SearchRequest, VectorPoint,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Resolve host and port from environment (with sane defaults).
    let host = std::env::var("CASPER_HOST").unwrap_or_else(|_| "http://127.0.0.1".to_string());
    let http_port: u16 = std::env::var("CASPER_HTTP_PORT").ok().and_then(|v| v.parse().ok()).unwrap_or(7222);

    let client = CasperClient::new(&host, http_port)?;

    println!("Casper Vector Database Client Example");

    // 1. Create a collection.
    println!("\nCreating collection...");
    client
        .create_collection("example_collection", CreateCollectionRequest { dim: 128, max_size: 10_000 })
        .await?;
    println!("Collection 'example_collection' created successfully");

    // 2. Upsert vectors (single batch call replaces the old per-id insert).
    println!("\nUpserting vectors...");
    let points: Vec<VectorPoint> = (1..=5)
        .map(|i| VectorPoint {
            id: i,
            vector: generate_random_vector(128, i as f32),
        })
        .collect();
    client.upsert_vectors("example_collection", points).await?;
    println!("Upserted 5 vectors");

    // 3. Mixed insert + delete in one call.
    println!("\nBatch update (insert + delete)...");
    let inserts: Vec<BatchInsertOperation> = (6..=10)
        .map(|i| BatchInsertOperation {
            id: i,
            vector: generate_random_vector(128, i as f32),
        })
        .collect();
    let batch_request = BatchUpdateRequest {
        insert: inserts,
        delete: vec![],
    };
    client.batch_update("example_collection", batch_request).await?;
    println!("Batch update completed");

    // 4. Create HNSW index. The server returns 202 immediately and builds
    //    the index in the background; in a real app you'd poll
    //    `get_collection` until `has_index` is true before searching.
    println!("\nCreating HNSW index...");
    let hnsw_request = CreateHNSWIndexRequest {
        hnsw: HNSWIndexConfig {
            metric: "inner-product".to_string(),
            quantization: "f32".to_string(),
            m: 16,
            m0: 32,
            ef_construction: 200,
        },
        normalization: Some(true),
    };
    client.create_hnsw_index("example_collection", hnsw_request).await?;
    println!("HNSW index build scheduled");

    // Poll until the index is ready.
    println!("Waiting for index build to finish...");
    loop {
        let info = client.get_collection("example_collection").await?;
        if info.has_index {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
    println!("Index ready");

    // 5. Search.
    println!("\nSearching for similar vectors...");
    let query_vector = generate_random_vector(128, 1.0);
    let results = client.search("example_collection", 30, SearchRequest { vector: query_vector }).await?;
    println!("Found {} similar vectors:", results.len());
    for (i, result) in results.iter().enumerate() {
        println!("  {}. ID: {}, Score: {:.4}", i + 1, result.id, result.score);
    }

    // 6. Get a specific vector.
    println!("\nGetting vector by ID...");
    if let Some(vector) = client.get_vector("example_collection", 1).await? {
        println!("Vector 1 retrieved: {} dimensions", vector.len());
    } else {
        println!("Vector 1 not found");
    }

    // 7. Delete vectors by id.
    println!("\nDeleting vectors...");
    client.delete_vectors("example_collection", vec![10]).await?;
    println!("Vector 10 deleted");

    // 8. Mute / unmute.
    println!("\nMuting and unmuting collection...");
    client.mute("example_collection").await?;
    client.unmute("example_collection").await?;
    println!("Mute/unmute roundtrip ok");

    // 9. Get collection info.
    println!("\nGetting collection information...");
    let collection_info = client.get_collection("example_collection").await?;
    println!("  - Name: {}", collection_info.name);
    println!("  - Dimension: {}", collection_info.dimension);
    println!("  - Mutable: {}", collection_info.mutable);
    println!("  - Has index: {}", collection_info.has_index);
    println!("  - Max size: {}", collection_info.max_size);
    println!("  - Size: {}", collection_info.size);
    println!("  - Status: {}", collection_info.status);
    if let Some(index) = collection_info.index {
        if let Some(hnsw) = index.hnsw {
            println!("  - Index: HNSW");
            println!("    - Metric: {}", hnsw.metric);
            println!("    - Quantization: {}", hnsw.quantization);
            println!("    - M: {}", hnsw.m);
            println!("    - M0: {}", hnsw.m0);
            println!("    - Ef construction: {}", hnsw.ef_construction);
            println!("    - Normalization: {}", index.normalization);
        }
    }

    // 10. List collections.
    println!("\nListing collections...");
    let collections = client.list_collections().await?;
    println!("Found {} collections:", collections.collections.len());
    for collection in collections.collections {
        println!(
            "  - {} (dim: {}, size: {}, mutable: {}, has_index: {})",
            collection.name, collection.dimension, collection.size, collection.mutable, collection.has_index,
        );
    }

    // 11. Delete index, then collection.
    println!("\nDeleting index...");
    client.delete_index("example_collection").await?;
    println!("Index deleted");

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
