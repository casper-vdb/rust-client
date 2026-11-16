use casper_client::{
    BatchUpdateRequest, CasperClient, CreateCollectionRequest, HNSWIndexConfig, IndexSpec, IndexState, ScoresRequest, SearchRequest, UpsertOperation,
};
use std::time::{Duration, Instant};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // host (with scheme) + HTTP port (Casper default is 7222)
    let client = CasperClient::new("http://localhost", 7222)?;

    // 1. Create a collection with the schema of its index.
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
            storage: None,
        },
    };
    let id = client.create_collection("example_collection", request).await?;
    println!("Collection created: {}", id);

    // 2. Write: upserts and deletes in one call.
    let upserts: Vec<UpsertOperation> = (1..=10)
        .map(|i| UpsertOperation {
            id: i,
            vector: generate_random_vector(128, i as f32),
        })
        .collect();
    let batch_request = BatchUpdateRequest { upserts, delete: vec![] };
    client.batch_update("example_collection", batch_request).await?;

    // 3. Start indexing (server returns 202, the build is asynchronous) and
    //    wait for `index.state` to become `ready`. `not_started` after a start
    //    means the build failed.
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

    // 4. Search, and score the query against chosen ids.
    let query_vector = generate_random_vector(128, 1.0);
    let results = client.search("example_collection", 5, SearchRequest { vector: query_vector.clone() }).await?;
    println!("Found {} results", results.len());

    let scores = client
        .scores(
            "example_collection",
            ScoresRequest {
                vector: query_vector,
                ids: vec![1, 2, 3],
            },
        )
        .await?;
    println!("Scored {} ids", scores.len());

    // 5. Get a vector by id.
    match client.get_vector("example_collection", 1).await? {
        Some(vector) => println!("Vector 1: {} components", vector.len()),
        None => println!("Vector 1 not found"),
    }

    // 6. Cleanup.
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
