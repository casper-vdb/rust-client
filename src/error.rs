use std::time::Duration;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, CasperError>;

#[derive(Error, Debug)]
pub enum CasperError {
    /// No HTTP answer came back — the connection failed or the request timed
    /// out — or its body could not be read. A write may still have been
    /// applied.
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

    /// 401 — no credential, or a bad one.
    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    /// 403 — the credential lacks access.
    #[error("Forbidden: {0}")]
    Forbidden(String),

    /// 404 — no such collection, or no such member for `remove_peer`. A
    /// vector the index does not hold is `Ok(None)` from `get_vector`.
    #[error("Not found: {0}")]
    NotFound(String),

    /// 405 as plain text — refused by the collection's state (e.g. it already
    /// exists, or its index is being built).
    #[error("Operation not allowed: {0}")]
    OperationNotAllowed(String),

    /// 405 with a JSON `{"error", "leader_addr"?}` body — a mutation reached a cluster node that is not
    /// the Raft leader. `leader_addr` is where to retry; `None` while no
    /// leader is known. The client does not redirect on its own.
    #[error("Not the leader: {message} (leader: {leader_addr:?})")]
    NotLeader { message: String, leader_addr: Option<String> },

    /// 409 — the request conflicts with the collection's state (e.g. a
    /// trigger on a collection with no index or with the mechanism off, or a
    /// policy change that lost a race).
    #[error("Conflict: {0}")]
    Conflict(String),

    /// 413 — the batch is over the server's limit; split it.
    #[error("Payload too large: {0}")]
    PayloadTooLarge(String),

    /// 429 — the write queue is full. `retry_after` is the server's
    /// `Retry-After`.
    #[error("Too many requests: {message}")]
    TooManyRequests { message: String, retry_after: Option<Duration> },

    /// 503 — the node cannot serve the request yet. `retry_after` is the
    /// server's `Retry-After`.
    #[error("Service unavailable: {message}")]
    ServiceUnavailable { message: String, retry_after: Option<Duration> },

    /// 504 — the write's outcome is unknown. Retrying is safe: writes are
    /// upserts by id.
    #[error("Gateway timeout: {0}")]
    GatewayTimeout(String),

    /// 507 — the node is short of free disk.
    #[error("Insufficient storage: {0}")]
    InsufficientStorage(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

impl CasperError {
    pub fn from_status(status: u16, message: String, retry_after: Option<Duration>) -> Self {
        match status {
            400 => CasperError::Client { status, message },
            401 => CasperError::Unauthorized(message),
            403 => CasperError::Forbidden(message),
            404 => CasperError::NotFound(message),
            405 => CasperError::OperationNotAllowed(message),
            409 => CasperError::Conflict(message),
            413 => CasperError::PayloadTooLarge(message),
            429 => CasperError::TooManyRequests { message, retry_after },
            503 => CasperError::ServiceUnavailable { message, retry_after },
            504 => CasperError::GatewayTimeout(message),
            507 => CasperError::InsufficientStorage(message),
            500..=599 => CasperError::Server { status, message },
            _ => CasperError::Unknown(format!("HTTP {}: {}", status, message)),
        }
    }

    /// The server's `Retry-After`, for a 429 or a 503 that carried one.
    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            CasperError::TooManyRequests { retry_after, .. } | CasperError::ServiceUnavailable { retry_after, .. } => *retry_after,
            _ => None,
        }
    }
}
