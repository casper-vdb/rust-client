pub mod client;
pub mod error;
pub mod models;

pub use client::CasperClient;
pub use error::{CasperError, Result};
pub use models::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_creation() {
        let client = CasperClient::new("http://localhost", 7222).unwrap();
        assert_eq!(client.base_url(), "http://localhost:7222/");
    }
}
