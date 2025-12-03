pub mod client;
pub mod error;
pub mod models;

pub use client::CasperClient;
pub use error::{CasperError, Result};
pub use models::*;

/// gRPC client types generated from `proto/matrix_service.proto`.
pub mod grpc {
    pub mod service {
        pub mod matrix_service {
            tonic::include_proto!("matrix_service");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_creation() {
        let client = CasperClient::new("http://localhost", 8080, 50051).unwrap();
        assert_eq!(client.base_url(), "http://localhost:8080/");
    }
}