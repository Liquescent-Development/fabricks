//! Daemon client for communicating with fabricksd over Unix socket.

use std::path::PathBuf;

use anyhow::{Context, Result};
use http_body_util::{BodyExt, Empty};
use hyper::body::Bytes;
use hyper::Request;
use hyper_util::rt::TokioIo;
use serde::de::DeserializeOwned;
use tokio::net::UnixStream;

/// Client for communicating with the fabricksd daemon.
pub struct DaemonClient {
    socket_path: PathBuf,
}

/// API response wrapper matching the daemon's response format.
#[derive(Debug, serde::Deserialize)]
#[serde(tag = "status")]
pub enum ApiResponse<T> {
    /// Successful response.
    #[serde(rename = "success")]
    Success {
        /// Response data.
        data: T,
    },

    /// Error response.
    #[serde(rename = "error")]
    Error {
        /// Error details.
        error: ApiError,
    },
}

/// API error details.
#[derive(Debug, serde::Deserialize)]
pub struct ApiError {
    /// Error code.
    pub code: String,
    /// Error message.
    pub message: String,
}

/// Daemon information response.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct DaemonInfo {
    /// Daemon version.
    pub version: String,
    /// API version.
    pub api_version: String,
    /// WASM runtime.
    pub runtime: String,
    /// Platform (os/arch).
    pub platform: String,
    /// When the daemon started.
    pub started_at: String,
    /// Human-readable uptime.
    pub uptime: String,
    /// Configuration info.
    pub config: DaemonConfigInfo,
}

/// Daemon configuration subset.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct DaemonConfigInfo {
    /// Socket path.
    pub socket: String,
    /// Data directory.
    pub data_dir: String,
    /// Maximum services.
    pub max_services: u32,
}

impl DaemonClient {
    /// Creates a new daemon client.
    ///
    /// Uses the default socket path based on the user's home directory.
    #[must_use]
    pub fn new() -> Self {
        let socket_path = default_socket_path();
        Self { socket_path }
    }

    /// Creates a daemon client with a custom socket path.
    #[must_use]
    pub fn with_socket(socket_path: PathBuf) -> Self {
        Self { socket_path }
    }

    /// Gets daemon information.
    ///
    /// # Errors
    ///
    /// Returns an error if the daemon is not running or communication fails.
    pub async fn info(&self) -> Result<DaemonInfo> {
        self.get("/v1/daemon/info").await
    }

    /// Performs a GET request to the daemon.
    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let stream = UnixStream::connect(&self.socket_path)
            .await
            .with_context(|| {
                format!(
                    "Failed to connect to daemon at {}. Is fabricksd running?",
                    self.socket_path.display()
                )
            })?;

        let io = TokioIo::new(stream);

        let (mut sender, conn) = hyper::client::conn::http1::handshake(io)
            .await
            .context("Failed to establish HTTP connection")?;

        // Spawn connection handler
        tokio::spawn(async move {
            if let Err(e) = conn.await {
                tracing::error!("Connection error: {e}");
            }
        });

        let request = Request::builder()
            .method("GET")
            .uri(path)
            .header("Host", "localhost")
            .body(Empty::<Bytes>::new())
            .context("Failed to build request")?;

        let response = sender
            .send_request(request)
            .await
            .context("Failed to send request")?;

        let body = response
            .into_body()
            .collect()
            .await
            .context("Failed to read response body")?
            .to_bytes();

        let api_response: ApiResponse<T> =
            serde_json::from_slice(&body).context("Failed to parse response")?;

        match api_response {
            ApiResponse::Success { data } => Ok(data),
            ApiResponse::Error { error } => {
                anyhow::bail!("{}: {}", error.code, error.message)
            }
        }
    }
}

impl Default for DaemonClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Gets the default socket path.
fn default_socket_path() -> PathBuf {
    dirs::home_dir().map_or_else(
        || PathBuf::from("/tmp/fabricks.sock"),
        |h| h.join(".fabricks/fabricks.sock"),
    )
}
