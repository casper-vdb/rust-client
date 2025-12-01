use crate::error::{CasperError, Result};
use crate::models::*;
use crate::grpc::service::matrix_service::{
    matrix_service_client::MatrixServiceClient,
    upload_matrix_request, MatrixData, MatrixHeader, UploadMatrixRequest,
};
use reqwest::Client;
use std::time::Duration;
use tokio_stream::wrappers::ReceiverStream;
use tonic::Request;
use url::Url;

/// Casper vector database client
#[derive(Debug, Clone)]
pub struct CasperClient {
    client: Client,
    base_url: Url,
}

impl CasperClient {
    /// Create a new Casper client
    pub fn new(base_url: &str) -> Result<Self> {
        let base_url = Url::parse(base_url)?;
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;
        
        Ok(Self { client, base_url })
    }

    /// Create a new Casper client with custom timeout
    pub fn with_timeout(base_url: &str, timeout: Duration) -> Result<Self> {
        let base_url = Url::parse(base_url)?;
        let client = Client::builder()
            .timeout(timeout)
            .build()?;
        
        Ok(Self { client, base_url })
    }

    /// Get the base URL
    pub fn base_url(&self) -> &str {
        self.base_url.as_str()
    }

    /// List all collections
    pub async fn list_collections(&self) -> Result<CollectionsListResponse> {
        let url = self.base_url.join("collections")?;
        let response = self.client.get(url).send().await?;
        
        self.handle_response(response).await
    }

    /// Get collection information
    pub async fn get_collection(&self, collection_name: &str) -> Result<CollectionInfo> {
        let url = self.base_url.join(&format!("collection/{}", collection_name))?;
        let response = self.client.get(url).send().await?;
        
        self.handle_response(response).await
    }

    /// Create a new collection
    pub async fn create_collection(
        &self,
        collection_name: &str,
        request: CreateCollectionRequest,
    ) -> Result<()> {
        let url = self.base_url.join(&format!("collection/{}", collection_name))?;
        let response = self
            .client
            .post(url)
            .query(&request)
            .header("Content-Type", "application/json")
            .send()
            .await?;
        
        self.handle_empty_response(response).await
    }

    /// Delete a collection
    pub async fn delete_collection(&self, collection_name: &str) -> Result<()> {
        let url = self.base_url.join(&format!("collection/{}", collection_name))?;
        let response = self.client.delete(url).send().await?;
        
        self.handle_empty_response(response).await
    }

    /// Insert a vector into a collection
    pub async fn insert_vector(
        &self,
        collection_name: &str,
        request: InsertRequest,
    ) -> Result<()> {
        let url = self.base_url.join(&format!("collection/{}/insert", collection_name))?;
        let response = self
            .client
            .post(url)
            .query(&[("id", request.id.to_string())])
            .header("Content-Type", "application/json")
            .json(&InsertVectorBody { vector: request.vector })
            .send()
            .await?;
        
        self.handle_empty_response(response).await
    }

    /// Delete a vector from a collection
    pub async fn delete_vector(
        &self,
        collection_name: &str,
        request: DeleteRequest,
    ) -> Result<()> {
        let url = self.base_url.join(&format!("collection/{}/delete", collection_name))?;
        let response = self
            .client
            .delete(url)
            .query(&[("id", request.id.to_string())])
            .header("Content-Type", "application/json")
            .send()
            .await?;
        
        self.handle_empty_response(response).await
    }

    /// Search for similar vectors
    pub async fn search(
        &self,
        collection_name: &str,
        limit: usize,
        request: SearchRequest,
    ) -> Result<SearchResponse> {
        let url = self.base_url.join(&format!("collection/{}/search", collection_name))?;
        let response = self
            .client
            .post(url)
            .query(&[("limit", limit.to_string())])
            .header("Content-Type", "application/json")
            .json(&SearchVectorBody { vector: request.vector })
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await?;
            return Err(self.parse_error_response(status.as_u16(), &text));
        }

        let bytes = response.bytes().await?;
        let buf = bytes.as_ref();

        // Binary format:
        // [u32 LE: count] followed by `count` * (u32 LE id, f32 LE score)
        if buf.len() < 4 {
            return Err(CasperError::InvalidResponse(
                "binary search response too short (missing count)".to_string(),
            ));
        }

        let mut offset = 0;
        let mut count_bytes = [0u8; 4];
        count_bytes.copy_from_slice(&buf[offset..offset + 4]);
        let count = u32::from_le_bytes(count_bytes) as usize;
        offset += 4;

        let expected_len = 4 + count * (4 + 4);
        if buf.len() < expected_len {
            return Err(CasperError::InvalidResponse(format!(
                "binary search response truncated: expected at least {} bytes, got {}",
                expected_len,
                buf.len()
            )));
        }

        let mut results = Vec::with_capacity(count);
        for _ in 0..count {
            let mut id_bytes = [0u8; 4];
            id_bytes.copy_from_slice(&buf[offset..offset + 4]);
            let id = u32::from_le_bytes(id_bytes);
            offset += 4;

            let mut score_bytes = [0u8; 4];
            score_bytes.copy_from_slice(&buf[offset..offset + 4]);
            let score = f32::from_le_bytes(score_bytes);
            offset += 4;

            results.push(SearchResult { id, score });
        }

        Ok(results)
    }

    /// Get vector by ID
    pub async fn get_vector(&self, collection_name: &str, id: u32) -> Result<Option<Vec<f32>>> {
        let url = self.base_url.join(&format!("collection/{}/vector/{}", collection_name, id))?;
        let response = self.client.get(url).send().await?;
        
        if response.status() == 404 {
            return Ok(None);
        }
        
        let vector_response: GetVectorResponse = self.handle_response(response).await?;
        Ok(Some(vector_response.vector))
    }

    /// Batch update operations
    pub async fn batch_update(
        &self,
        collection_name: &str,
        id: u32,
        request: BatchUpdateRequest,
    ) -> Result<()> {
        let url = self.base_url.join(&format!("collection/{}/update", collection_name))?;
        let response = self
            .client
            .post(url)
            .query(&[("id", id.to_string())])
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;
        
        self.handle_empty_response(response).await
    }

    /// Create IVF index
    pub async fn create_ivf_index(
        &self,
        collection_name: &str,
        request: CreateIVFIndexRequest,
    ) -> Result<()> {
        let url = self.base_url.join(&format!("collections/{}/index", collection_name))?;
        let response = self
            .client
            .post(url)
            .json(&request)
            .send()
            .await?;
        
        self.handle_empty_response(response).await
    }

    /// Create HNSW index
    pub async fn create_hnsw_index(
        &self,
        collection_name: &str,
        has_normalization: bool,
        request: CreateHNSWIndexRequest,
    ) -> Result<()> {
        let url = self.base_url.join(&format!("collection/{}/index", collection_name))?;
        let response = self
            .client
            .post(url)
            .query(&[("has_normalization", has_normalization.to_string())])
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;
        
        self.handle_empty_response(response).await
    }

    /// Delete index from collection
    pub async fn delete_index(&self, collection_name: &str) -> Result<()> {
        let url = self.base_url.join(&format!("collection/{}/index", collection_name))?;
        let response = self.client.delete(url).send().await?;
        
        self.handle_empty_response(response).await
    }

    /// Upload a matrix via gRPC streaming.
    ///
    /// - `grpc_addr`: gRPC endpoint, e.g. "http://127.0.0.1:50051"
    /// - `matrix_name`: name of the matrix to create/overwrite
    /// - `dimension`: vector dimensionality
    /// - `vectors`: flat list of all vectors, concatenated row-wise
    /// - `chunk_floats`: number of f32 values per chunk (must be >= dimension)
    pub async fn upload_matrix_grpc(
        &self,
        grpc_addr: &str,
        matrix_name: &str,
        dimension: usize,
        vectors: Vec<f32>,
        chunk_floats: usize,
    ) -> Result<UploadMatrixResult> {
        use crate::error::CasperError;

        if dimension == 0 {
            return Err(CasperError::InvalidResponse(
                "dimension must be greater than 0".to_string(),
            ));
        }

        if vectors.len() % dimension != 0 {
            return Err(CasperError::InvalidResponse(format!(
                "vector buffer length {} is not divisible by dimension {}",
                vectors.len(),
                dimension
            )));
        }

        let chunk_floats = if chunk_floats < dimension {
            dimension
        } else {
            chunk_floats
        };

        let total_floats = vectors.len();
        let total_chunks = (total_floats + chunk_floats - 1) / chunk_floats;

        let mut client = MatrixServiceClient::connect(grpc_addr.to_string())
            .await
            .map_err(|e| CasperError::Grpc(e.to_string()))?;

        let (tx, rx) = tokio::sync::mpsc::channel::<UploadMatrixRequest>(4);

        // Spawn producer task to send header + chunks
        let name = matrix_name.to_string();
        let vectors_clone = vectors.clone();
        tokio::spawn(async move {
            // Header first
            let max_vectors_per_chunk = (chunk_floats / dimension).max(1) as u32;
            let header = MatrixHeader {
                name: name.clone(),
                dimension: dimension as u32,
                total_chunks: total_chunks as u32,
                max_vectors_per_chunk,
            };
            let header_msg = UploadMatrixRequest {
                payload: Some(upload_matrix_request::Payload::Header(header)),
            };
            if tx.send(header_msg).await.is_err() {
                return;
            }

            // Then data chunks
            for chunk_idx in 0..total_chunks {
                let start = chunk_idx * chunk_floats;
                let end = (start + chunk_floats).min(total_floats);
                let slice = &vectors_clone[start..end];

                let data = MatrixData {
                    chunk_index: chunk_idx as u32,
                    vector: slice.to_vec(),
                };
                let msg = UploadMatrixRequest {
                    payload: Some(upload_matrix_request::Payload::Data(data)),
                };

                if tx.send(msg).await.is_err() {
                    break;
                }
            }
        });

        let request = Request::new(ReceiverStream::new(rx));
        let response = client
            .upload_matrix(request)
            .await
            .map_err(|e| CasperError::Grpc(e.to_string()))?
            .into_inner();

        Ok(UploadMatrixResult {
            success: true,
            message: format!(
                "Successfully uploaded {} vectors in {} chunks",
                response.total_vectors, response.total_chunks
            ),
            total_vectors: response.total_vectors,
            total_chunks: response.total_chunks,
        })
    }

    /// Delete a matrix by name (HTTP)
    pub async fn delete_matrix(&self, name: &str) -> Result<()> {
        let url = self.base_url.join(&format!("matrix/{}", name))?;
        let response = self
            .client
            .delete(url)
            .header("Content-Type", "application/json")
            .send()
            .await?;

        self.handle_empty_response(response).await
    }

    /// List all matrices (HTTP)
    pub async fn list_matrices(&self) -> Result<Vec<MatrixInfo>> {
        let url = self.base_url.join("matrix/list")?;
        let response = self
            .client
            .get(url)
            .header("Content-Type", "application/json")
            .send()
            .await?;

        self.handle_response(response).await
    }

    /// Get matrix info by name (HTTP)
    pub async fn get_matrix_info(&self, name: &str) -> Result<MatrixInfo> {
        let url = self.base_url.join(&format!("matrix/{}", name))?;
        let response = self
            .client
            .get(url)
            .header("Content-Type", "application/json")
            .send()
            .await?;

        self.handle_response(response).await
    }

    /// Create a PQ entry
    pub async fn create_pq(
        &self,
        name: &str,
        request: CreatePqRequest,
    ) -> Result<()> {
        let url = self.base_url.join(&format!("pq/{}", name))?;
        let response = self
            .client
            .post(url)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        self.handle_empty_response(response).await
    }

    /// Delete a PQ entry
    pub async fn delete_pq(&self, name: &str) -> Result<()> {
        let url = self.base_url.join(&format!("pq/{}", name))?;
        let response = self
            .client
            .delete(url)
            .header("Content-Type", "application/json")
            .send()
            .await?;

        self.handle_empty_response(response).await
    }

    /// List all PQs
    pub async fn list_pqs(&self) -> Result<Vec<PqInfo>> {
        let url = self.base_url.join("pq/list")?;
        let response = self
            .client
            .get(url)
            .header("Content-Type", "application/json")
            .send()
            .await?;

        self.handle_response(response).await
    }

    /// Get PQ info by name
    pub async fn get_pq(&self, name: &str) -> Result<PqInfo> {
        let url = self.base_url.join(&format!("pq/{}", name))?;
        let response = self
            .client
            .get(url)
            .header("Content-Type", "application/json")
            .send()
            .await?;

        self.handle_response(response).await
    }

    /// Handle JSON response
    async fn handle_response<T>(&self, response: reqwest::Response) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let status = response.status();
        let text = response.text().await?;
        
        if status.is_success() {
            serde_json::from_str(&text).map_err(|e| CasperError::InvalidResponse(format!(
                "Failed to parse response: {} - {}", e, text
            )))
        } else {
            Err(self.parse_error_response(status.as_u16(), &text))
        }
    }

    /// Handle empty response (204 No Content)
    async fn handle_empty_response(&self, response: reqwest::Response) -> Result<()> {
        let status = response.status();
        
        if status.is_success() {
            Ok(())
        } else {
            let text = response.text().await?;
            Err(self.parse_error_response(status.as_u16(), &text))
        }
    }


    /// Parse error response
    fn parse_error_response(&self, status: u16, text: &str) -> CasperError {
        // Try to parse as JSON error response
        if let Ok(error_json) = serde_json::from_str::<serde_json::Value>(text) {
            if let Some(message) = error_json.get("error").and_then(|v| v.as_str()) {
                return CasperError::from_status(status, message.to_string());
            }
        }
        
        // Fallback to status-based error
        CasperError::from_status(status, text.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = CasperClient::new("http://localhost:8080").unwrap();
        assert_eq!(client.base_url(), "http://localhost:8080/");
    }

    #[test]
    fn test_client_creation_with_invalid_url() {
        let result = CasperClient::new("invalid-url");
        assert!(result.is_err());
    }

    #[test]
    fn test_create_collection_url() {
        let client = CasperClient::new("http://localhost:8080").unwrap();
        
        // Test URL construction
        let url = client.base_url.join("collection/test_collection").unwrap();
        assert_eq!(url.as_str(), "http://localhost:8080/collection/test_collection");
    }

    #[test]
    fn test_insert_vector_url() {
        let client = CasperClient::new("http://localhost:8080").unwrap();
        
        // Test URL construction for insert
        let url = client.base_url.join("collection/b/insert").unwrap();
        assert_eq!(url.as_str(), "http://localhost:8080/collection/b/insert");
    }

    #[test]
    fn test_batch_update_url() {
        let client = CasperClient::new("http://localhost:8080").unwrap();
        
        // Test URL construction for batch update
        let url = client.base_url.join("collection/alex/batch_update").unwrap();
        assert_eq!(url.as_str(), "http://localhost:8080/collection/alex/batch_update");
    }

    #[test]
    fn test_create_hnsw_index_url() {
        let client = CasperClient::new("http://localhost:8080").unwrap();
        
        // Test URL construction for HNSW index creation
        let url = client.base_url.join("collection/alex/index").unwrap();
        assert_eq!(url.as_str(), "http://localhost:8080/collection/alex/index");
    }

    #[test]
    fn test_search_url() {
        let client = CasperClient::new("http://localhost:8080").unwrap();
        
        // Test URL construction for search
        let url = client.base_url.join("collection/alex/search").unwrap();
        assert_eq!(url.as_str(), "http://localhost:8080/collection/alex/search");
    }

    #[test]
    fn test_delete_vector_url() {
        let client = CasperClient::new("http://localhost:8080").unwrap();
        
        // Test URL construction for delete vector
        let url = client.base_url.join("collection/alex/delete").unwrap();
        assert_eq!(url.as_str(), "http://localhost:8080/collection/alex/delete");
    }
}
