use casper_client::{
    CasperClient,
    CreateCollectionRequest,
    InsertRequest,
    SearchRequest,
    BatchUpdateRequest,
    BatchInsertOperation,
    CreateHNSWIndexRequest,
    HNSWIndexConfig,
    CreatePqRequest,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Resolve host and ports from environment (with sane defaults)
    let host = std::env::var("CASPER_HOST")
        .unwrap_or_else(|_| "http://127.0.0.1".to_string());
    let http_port: u16 = std::env::var("CASPER_HTTP_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8080);
    let grpc_port: u16 = std::env::var("CASPER_GRPC_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(50051);

    // Create a client for HTTP & gRPC APIs
    let client = CasperClient::new(&host, http_port, grpc_port)?;
    
    println!("Casper Vector Database Client Example");

    // 1. Create a collection
    println!("\nCreating collection...");
    let create_request = CreateCollectionRequest {
        dim: 128,
        max_size: 10000,
    };
    client.create_collection("example_collection", create_request).await?;
    println!("Collection 'example_collection' created successfully");

    // 2. Insert some vectors
    println!("\nInserting vectors...");
    for i in 1..=5 {
        let vector = generate_random_vector(128, i as f32);
        let insert_request = InsertRequest {
            id: i,
            vector,
        };
        client.insert_vector("example_collection", insert_request).await?;
        println!("Vector {} inserted", i);
    }

    // 3. Batch insert more vectors
    println!("\nBatch inserting vectors...");
    let mut inserts = Vec::new();
    for i in 6..=10 {
        let vector = generate_random_vector(128, i as f32);
        inserts.push(BatchInsertOperation { id: i, vector });
    }
    let batch_request = BatchUpdateRequest { insert: inserts, delete: vec![] };
    client.batch_update("example_collection", batch_request).await?;
    println!("Batch insert completed");

    // 4. Create HNSW index
    println!("\nCreating HNSW index...");
    let hnsw_request = CreateHNSWIndexRequest {
        index: "hnsw".to_string(),
        config: HNSWIndexConfig {
            metric: "inner-product".to_string(),
            quantization: "i8".to_string(),
            has_normalization: Some(true),
            max_elements: 600000,
            m: 16,
            m0: 32,
            ef_construction: 200,
            ef: 50,
        },
    };
    client.create_hnsw_index("example_collection", true, hnsw_request).await?;
    println!("HNSW index created");

    // 5. Search for similar vectors
    println!("\nSearching for similar vectors...");
    let query_vector = generate_random_vector(128, 1.0);
    let search_request = SearchRequest {
        vector: query_vector,
        limit: Some(5),
    };
    let results = client.search("example_collection", 30, search_request).await?;

    println!("Found {} similar vectors:", results.len());
    for (i, result) in results.iter().enumerate() {
        println!("  {}. ID: {}, Score: {:.4}", i + 1, result.id, result.score);
    }

    // 6. Get a specific vector
    println!("\nGetting vector by ID...");
    if let Some(vector) = client.get_vector("example_collection", 1).await? {
        println!("Vector 1 retrieved: {} dimensions", vector.len());
    } else {
        println!("Vector 1 not found");
    }

    // 7. Delete a vector
    println!("\nDeleting vector...");
    client.delete_vector("example_collection", casper_client::DeleteRequest { id: 10 }).await?;
    println!("Vector 10 deleted");

    // 8. Get collection information
    println!("\nGetting collection information...");
    let collection_info = client.get_collection("example_collection").await?;
    println!("Collection info retrieved:");
    println!("  - Name: {}", collection_info.name);
    println!("  - Dimension: {}", collection_info.dimension);
    println!("  - Mutable: {}", collection_info.mutable);
    println!("  - Has index: {}", collection_info.has_index);
    println!("  - Max size: {}", collection_info.max_size);
    if let Some(index) = collection_info.index {
        println!("  - Index type: {}", index.index_type);
        println!("  - Metric: {}", index.metric);
        println!("  - Quantization: {}", index.quantization);
    }

    // 9. List collections
    println!("\nListing collections...");
    let collections = client.list_collections().await?;
    println!("Found {} collections:", collections.collections.len());
    for collection in collections.collections {
        println!("  - {} (dim: {}, mutable: {}, has_index: {})",
                 collection.name,
                 collection.dimension,
                 collection.mutable,
                 collection.has_index);
    }

    // 10. Delete the index
    println!("\nDeleting index...");
    client.delete_index("example_collection").await?;
    println!("Index deleted successfully");

    // 11. Delete the collection
    println!("\nDeleting collection...");
    client.delete_collection("example_collection").await?;
    println!("Collection 'example_collection' deleted successfully");

    // A. Matrix operations (gRPC upload + HTTP management)
    println!("\nCreating matrices via gRPC...");

    let dim = 3usize;
    let m1_name = "example_matrix_1";
    let m2_name = "example_matrix_2";

    // Two simple 3D vectors for each matrix
    let m1_vectors: Vec<f32> = vec![
        1.0, 2.0, 3.0,
        4.0, 5.0, 6.0,
    ];
    let m2_vectors: Vec<f32> = vec![
        0.1, 0.2, 0.3,
        0.4, 0.5, 0.6,
    ];

    let res1 = client
        .upload_matrix(m1_name, dim, m1_vectors.clone(), 6)
        .await?;
    println!("Uploaded matrix '{}' via gRPC: {}", m1_name, res1.message);

    let res2 = client
        .upload_matrix(m2_name, dim, m2_vectors.clone(), 6)
        .await?;
    println!("Uploaded matrix '{}' via gRPC: {}", m2_name, res2.message);

    println!("\nListing matrices...");
    let matrices = client.list_matrices().await?;
    println!("Found {} matrices:", matrices.len());
    for m in &matrices {
        println!(
            "  - {} (dim: {}, len: {}, enabled: {})",
            m.name, m.dim, m.len, m.enabled
        );
    }

    println!("\nGetting matrix info individually...");
    let info1 = client.get_matrix_info(m1_name).await?;
    println!(
        "  - {}: dim={}, len={}, enabled={}",
        info1.name, info1.dim, info1.len, info1.enabled
    );
    let info2 = client.get_matrix_info(m2_name).await?;
    println!(
        "  - {}: dim={}, len={}, enabled={}",
        info2.name, info2.dim, info2.len, info2.enabled
    );

    // B. PQ operations (HTTP)
    println!("\nCreating PQ...");
    let pq_name = "example_pq";
    // Use the two matrices we just created as PQ codebooks.
    // Each has dim=3, so total PQ dim is 6.
    let pq_request = CreatePqRequest {
        dim: dim * 2, // sum of codebooks dims (3 + 3)
        codebooks: vec![m1_name.to_string(), m2_name.to_string()],
    };

    match client.create_pq(pq_name, pq_request).await {
        Ok(()) => println!("PQ '{}' created", pq_name),
        Err(e) => {
            println!("Failed to create PQ '{}': {}", pq_name, e);
        }
    }

    println!("\nListing PQs...");
    let pqs = client.list_pqs().await?;
    println!("Found {} PQs:", pqs.len());
    for pq in &pqs {
        println!(
            "  - {} (dim: {}, codebooks: {:?}, enabled: {})",
            pq.name, pq.dim, pq.codebooks, pq.enabled
        );
    }

    println!("\nGetting PQ info individually...");
    match client.get_pq(pq_name).await {
        Ok(pq_info) => {
            println!(
                "  - {}: dim={}, codebooks={:?}, enabled={}",
                pq_info.name, pq_info.dim, pq_info.codebooks, pq_info.enabled
            );
        }
        Err(e) => {
            println!("Failed to get PQ '{}': {}", pq_name, e);
        }
    }

    println!("\nDeleting PQ '{}'...", pq_name);
    match client.delete_pq(pq_name).await {
        Ok(()) => println!("PQ '{}' deleted", pq_name),
        Err(e) => println!("Failed to delete PQ '{}': {}", pq_name, e),
    }

    // C. Delete matrices after PQ is gone
    println!("\nDeleting matrices '{}' and '{}'...", m1_name, m2_name);
    client.delete_matrix(m1_name).await?;
    client.delete_matrix(m2_name).await?;
    println!("Matrices '{}' and '{}' deleted", m1_name, m2_name);

    let matrices_after = client.list_matrices().await?;
    println!("Matrices after deletion ({} total):", matrices_after.len());
    for m in &matrices_after {
        println!(
            "  - {} (dim: {}, len: {}, enabled: {})",
            m.name, m.dim, m.len, m.enabled
        );
    }
    
    println!("\nExample completed successfully!");
    Ok(())
}

/// Generate a random vector for demonstration
fn generate_random_vector(dim: usize, seed: f32) -> Vec<f32> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    let mut vector = Vec::with_capacity(dim);
    for i in 0..dim {
        let mut hasher = DefaultHasher::new();
        (seed * 1000.0 + i as f32).to_bits().hash(&mut hasher);
        let hash = hasher.finish();
        let value = (hash as f32 / u64::MAX as f32) * 2.0 - 1.0; // Normalize to [-1, 1]
        vector.push(value);
    }
    
    // Normalize the vector
    let norm: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in &mut vector {
            *value /= norm;
        }
    }
    
    vector
}
