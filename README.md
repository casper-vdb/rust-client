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

The package is `casper-vdb`; its library is imported as `casper_client`.

## Quick start

```rust
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
```

## API surface

- Client: `CasperClient::new`, `CasperClient::with_timeout`, `with_api_key` (sends the `api-key` header, needed when the server has API keys configured)
- Collections: `create_collection` (takes the index schema and policy, returns the collection's uuid), `delete_collection`, `get_collection`, `list_collections` (names only; use `get_collection` for details)
- Writes: `batch_update` (upserts and deletes in one call; ids are `u64`)
- Index: `start_indexing` (the build is asynchronous — the server returns 202; poll `get_collection` until `index.state` is `IndexState::Ready`, and treat `IndexState::NotStarted` after a start as a failed build), `update_index_policy` (snapshot and compaction policy, before indexing starts as well), `trigger_snapshot`, `trigger_compaction` (a 409 when the mechanism is off on the node — see `get_config().storage.index` — or in the index's policy). The schema never changes; the index lives until its collection is dropped.
- Search: `search`, `search_with_ef`, `scores` (the query scored against given ids, in their order; ids the index does not hold are left out). The binary response wire format is decoded automatically.
- Vectors: `get_vector` returns `Ok(None)` only when the index holds no such id; a missing collection is an error. The vector comes back as the index stores it: `VectorComponents::F32` for an `f32` index, `VectorComponents::I8` (quantized values in `[-100, 100]`, not dequantized) for an `i8` index.
- Cluster: `get_cluster` (node role, Raft state and members), `remove_peer` (must be sent to the leader), `set_transfer_limit` (held in memory until restart)
- Configuration & metrics: `get_config` (the startup configuration, grouped as the config file is), `get_metrics` (Prometheus text format)
- Health: `health` (liveness), `ready(quorum)` (readiness; `quorum = true` also requires the node to count for quorum)

`CreateCollectionRequest.index` takes an optional `storage` (`IndexStorageSpec`: `range_size_bytes`, `snapshot`, `compaction`); every key left out takes the server's default. In `CompactionPatch`, `max_chain_depth` has three states: `None` leaves it as it is, `Some(None)` turns the depth rule off, `Some(Some(n))` sets it.

Errors are `CasperError` variants per status: `Client` (400), `Unauthorized` (401), `Forbidden` (403), `NotFound` (404), `OperationNotAllowed` (405), `NotLeader { leader_addr }` (405 from a cluster node that is not the Raft leader — retry against `leader_addr`), `Conflict` (409), `PayloadTooLarge` (413 — split the batch), `TooManyRequests` (429) and `ServiceUnavailable` (503) with `retry_after`, `GatewayTimeout` (504 — the outcome is unknown, retrying is safe), `InsufficientStorage` (507), `Server` (other 5xx). The client never retries or redirects on its own.

## Examples

Run the example provided in this repository:

```bash
cargo run --example example
```

The example demonstrates:
- Cluster and configuration info (role, Raft state, one config value)
- Collection management (create with an index schema, info, list, delete)
- Writes (upserts, and upserts with deletes in one call)
- Indexing (start, wait for `ready`, update the policy, trigger a snapshot and a compaction where the node runs them)
- Reads (search, scores, get a vector by id)

## License

Licensed under the [Apache License Version 2.0](LICENSE).
