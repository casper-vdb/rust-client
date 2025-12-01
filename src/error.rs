use thiserror::Error;

pub type Result<T> = std::result::Result<T, CasperError>;

#[derive(Error, Debug)]
pub enum CasperError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    
    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),
    
    #[error("URL parsing error: {0}")]
    Url(#[from] url::ParseError),
    
    #[error("Server error: {status} - {message}")]
    Server { status: u16, message: String },
    
    #[error("Client error: {status} - {message}")]
    Client { status: u16, message: String },
    
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
    
    #[error("Collection not found: {0}")]
    CollectionNotFound(String),
    
    #[error("Index creation in progress")]
    IndexCreationInProgress,
    
    #[error("Operation not allowed: {0}")]
    OperationNotAllowed(String),
    
    #[error("Invalid vector dimension: expected {expected}, got {actual}")]
    InvalidDimension { expected: usize, actual: usize },
    
    #[error("Vector ID exceeds collection max size: {id}")]
    IdExceedsMaxSize { id: u32 },
    
    #[error("Zero-norm vectors are not allowed")]
    ZeroNormVector,
    
    #[error("Collection is not mutable")]
    CollectionNotMutable,
    
    #[error("Index already exists")]
    IndexAlreadyExists,
    
    #[error("gRPC error: {0}")]
    Grpc(String),
    
    #[error("Unknown error: {0}")]
    Unknown(String),
}

impl CasperError {
    pub fn from_status(status: u16, message: String) -> Self {
        match status {
            400 => CasperError::Client { status, message },
            404 => CasperError::CollectionNotFound(message),
            405 => CasperError::OperationNotAllowed(message),
            409 => CasperError::IndexAlreadyExists,
            500..=599 => CasperError::Server { status, message },
            _ => CasperError::Unknown(format!("HTTP {}: {}", status, message)),
        }
    }
}
