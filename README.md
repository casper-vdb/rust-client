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
    BatchInsertOperation, BatchUpdateRequest, CasperClient, CreateCollectionRequest, CreateHNSWIndexRequest, HNSWIndexConfig, SearchRequest, VectorPoint,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // host (with scheme) + HTTP port (Casper default since v0.1.0 is 7222)
    let client = CasperClient::new("http://localhost", 7222)?;

    // 1. Create a collection
    client
        .create_collection("example_collection", CreateCollectionRequest { dim: 128, max_size: 10_000 })
        .await?;

    // 2. Upsert some vectors (single call replaces the old per-id insert).
    let points: Vec<VectorPoint> = (1..=5)
        .map(|i| VectorPoint {
            id: i,
            vector: generate_random_vector(128, i as f32),
        })
        .collect();
    client.upsert_vectors("example_collection", points).await?;

    // 3. Mixed insert+delete in one batch.
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

    // 4. Create HNSW index (server returns 202, build is asynchronous).
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
    let query_vector = generate_random_vector(128, 1.0);
    let results = client.search("example_collection", 30, SearchRequest { vector: query_vector }).await?;

    println!("Found {} results", results.len());

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
```

## API surface

- Collections: `create_collection`, `delete_collection`, `get_collection`, `list_collections`
- Vectors: `upsert_vectors`, `delete_vectors`, `batch_update`, `get_vector`
- Mute / unmute: `mute`, `unmute`
- Index: `create_hnsw_index`, `delete_index` (index build is asynchronous — the server returns 202 and builds in the background; poll `get_collection` for `has_index` if you need to wait)
- Search: `search`, `search_with_ef` (binary response wire format is decoded automatically)

## Examples

Run the example provided in this repository:

```bash
cargo run --example example
```

The example demonstrates:
- Collection management (create, delete, list, info)
- Vector operations (upsert, batch update, search, get, delete)
- Mute / unmute
- Index management (create / delete HNSW)

## License

Licensed under the [Apache License Version 2.0](LICENSE).
