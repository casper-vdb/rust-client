use casper_client::{
    CasperClient,
    CreateCollectionRequest,
    InsertRequest,
    SearchRequest,
    BatchUpdateRequest,
    BatchInsertOperation,
    CreateHNSWIndexRequest,
    HNSWIndexConfig,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // host (with scheme) + HTTP and gRPC ports
    let client = CasperClient::new("http://localhost", 8080, 50051)?;

    // 1 Create a collection
    client
        .create_collection("example_collection", CreateCollectionRequest {
            dim: 128,
            max_size: 10_000,
        })
        .await?;

    // 2 Insert some vectors
    for i in 1..=5 {
        let vector = generate_random_vector(128, i as f32);
        let insert_request = InsertRequest { id: i, vector };
        client.insert_vector("example_collection", insert_request).await?;
    }

    // 3 Batch insert more vectors
    let mut inserts = Vec::new();
    for i in 6..=10 {
        let vector = generate_random_vector(128, i as f32);
        inserts.push(BatchInsertOperation { id: i, vector });
    }
    let batch_request = BatchUpdateRequest { insert: inserts, delete: vec![] };
    client.batch_update("example_collection", batch_request).await?;

    // 4 Create HNSW index
    let hnsw_request = CreateHNSWIndexRequest {
        hnsw: HNSWIndexConfig {
            metric: "inner-product".to_string(),
            quantization: "f32".to_string(),
            m: 16,
            m0: 32,
            ef_construction: 200,
            pq_name: None,
        },
        normalization: Some(true),
    };
    client.create_hnsw_index("example_collection", hnsw_request).await?;

    // 5 Search for similar vectors
    let query_vector = generate_random_vector(128, 1.0);
    let results = client
        .search(
            "example_collection",
            30,
            SearchRequest { vector: query_vector, limit: Some(5) },
        )
        .await?;

    println!("Found {} results", results.len());

    // 6 Delete the collection
    println!("\nDeleting collection...");
    client.delete_collection("example_collection").await?;
    println!("Collection 'example_collection' deleted successfully");

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
