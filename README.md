# Casper Rust client

Client library for the [Casper Vector Database](https://github.com/casper-vdb/Casper) written in Rust.

## Installation

If the crate is published on crates.io:

```bash
cargo add casper-vdb
```

If not yet published, use a git dependency:

```toml
[dependencies]
casper-vdb = { git = "https://github.com/casper-vdb/rust-client" }
```

## Quick start

```rust
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
```

[//]: # (## Matrix upload over gRPC)

[//]: # ()
[//]: # (The client exposes a streaming gRPC method to upload matrices efficiently, generated from `proto/matrix_service.proto`.)

[//]: # ()
[//]: # (```rust)

[//]: # (use casper_client::CasperClient;)

[//]: # ()
[//]: # (#[tokio::main])

[//]: # (async fn main&#40;&#41; -> Result<&#40;&#41;, Box<dyn std::error::Error>> {)

[//]: # (    let client = CasperClient::new&#40;"http://localhost:8080"&#41;?;)

[//]: # ()
[//]: # (    let grpc_addr = "http://127.0.0.1:50051";)

[//]: # (    let name = "example_matrix";)

[//]: # (    let dim = 3usize;)

[//]: # (    let vectors: Vec<f32> = vec![1.0, 2.0, 3.0,  4.0, 5.0, 6.0]; // two 3D vectors)

[//]: # ()
[//]: # (    let res = client)

[//]: # (        .upload_matrix_grpc&#40;grpc_addr, name, dim, vectors, /*chunk_floats*/ 6&#41;)

[//]: # (        .await?;)

[//]: # ()
[//]: # (    println!&#40;"{}", res.message&#41;;)

[//]: # (    Ok&#40;&#40;&#41;&#41;)

[//]: # (})

[//]: # (```)

[//]: # (Notes:)
[//]: # (- gRPC client is built with `tonic`. The repository includes a build script that uses a vendored `protoc`, so a system-wide `protoc` is not required.)

## Examples

Run the example provided in this repository:

```bash
cargo run --example example
```

The example demonstrates:
- Collection management (create, delete, list, info)
- Vector operations (insert, batch update, search, get, delete)
- Index management (create/delete HNSW)
- Matrix operations (gRPC upload, HTTP listing/info/delete)
- PQ operations (create/list/get/delete)

## License

Licensed under the [Apache License Version 2.0](LICENSE).