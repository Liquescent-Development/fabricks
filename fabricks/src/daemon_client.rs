//! Daemon client for communicating with fabricksd over Unix socket.

use std::path::PathBuf;

use anyhow::{Context, Result};
use http_body_util::{BodyExt, Empty, Full};
use hyper::Request;
use hyper::body::Bytes;
use hyper_util::rt::TokioIo;
use serde::Serialize;
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

/// Service information.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ServiceInfo {
    /// Service ID.
    pub id: String,
    /// Service name.
    pub name: String,
    /// Service version.
    pub version: String,
    /// Current state.
    pub state: String,
    /// Replica information.
    pub replicas: ReplicaState,
    /// When the service was created.
    pub created_at: String,
    /// Optional mortar project.
    #[serde(default)]
    pub mortar_project: Option<String>,
}

/// Replica state.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ReplicaState {
    /// Desired replicas.
    pub desired: usize,
    /// Ready replicas.
    pub ready: usize,
    /// Running replicas.
    pub running: usize,
    /// Failed replicas.
    pub failed: usize,
}

/// List services response.
#[derive(Debug, serde::Deserialize)]
pub struct ListServicesResponse {
    /// List of services.
    pub services: Vec<ServiceInfo>,
    /// Total count.
    pub total: usize,
}

/// Detailed service information.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct ServiceDetail {
    /// Service ID.
    pub id: String,
    /// Service name.
    pub name: String,
    /// Service version.
    pub version: String,
    /// Current state.
    pub state: String,
    /// Replica information.
    pub replicas: ReplicaState,
    /// Service configuration.
    pub config: ServiceConfigDetail,
    /// When the service was created.
    pub created_at: String,
    /// When the service was last updated.
    pub updated_at: String,
    /// Last error message.
    #[serde(default)]
    pub last_error: Option<String>,
    /// Running instances.
    #[serde(default)]
    pub instances: Vec<InstanceInfo>,
}

/// Service configuration details.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct ServiceConfigDetail {
    /// Service name.
    pub name: String,
    /// Service version.
    pub version: String,
    /// Service type.
    pub service_type: String,
    /// Path to WASM module.
    pub wasm_path: String,
    /// WASM digest.
    pub wasm_digest: String,
    /// Networks.
    #[serde(default)]
    pub networks: Vec<String>,
    /// Mortar project.
    #[serde(default)]
    pub mortar_project: Option<String>,
}

/// Instance information.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct InstanceInfo {
    /// Instance ID.
    pub id: String,
    /// Instance state.
    pub state: String,
    /// When started.
    #[serde(default)]
    pub started_at: Option<String>,
}

/// Create service response.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct CreateServiceResponse {
    /// Created service ID.
    pub id: String,
    /// Service name.
    pub name: String,
}

/// Run Fabrickfile request.
#[derive(Debug, serde::Serialize)]
pub struct RunFabrickfileRequest {
    /// Path to Fabrickfile.
    pub fabrickfile_path: PathBuf,
    /// Optional path to pre-built WASM.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wasm_path: Option<PathBuf>,
}

/// Run module request (by tag or registry reference).
#[derive(Debug, serde::Serialize)]
pub struct RunModuleRequest {
    /// Module reference (tag like "my-module:1.0.0" or registry reference).
    pub reference: String,
    /// Additional arguments to pass to the module.
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variable overrides.
    #[serde(default)]
    pub env_vars: Vec<(String, String)>,
    /// Whether to disable capability enforcement.
    #[serde(default)]
    pub no_capabilities: bool,
}

/// Scale service request.
#[derive(Debug, serde::Serialize)]
pub struct ScaleServiceRequest {
    /// Target number of replicas.
    pub replicas: usize,
}

/// Deploy mortar request.
#[derive(Debug, serde::Serialize)]
pub struct DeployMortarRequest {
    /// Path to mortar file.
    pub mortar_path: PathBuf,
}

/// Deploy mortar response.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct DeployMortarResponse {
    /// Project name.
    pub project: String,
    /// Created service IDs.
    pub service_ids: Vec<String>,
    /// Total services.
    pub total: usize,
}

/// List projects response.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct ListProjectsResponse {
    /// Project names.
    pub projects: Vec<String>,
    /// Total count.
    pub total: usize,
}

/// Project services response.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct ProjectServicesResponse {
    /// Project name.
    pub project: String,
    /// Services in the project.
    pub services: Vec<ServiceInfo>,
    /// Total count.
    pub total: usize,
}

// ==================== Network Types ====================

/// Create network request.
#[derive(Debug, serde::Serialize)]
pub struct CreateNetworkRequest {
    /// Network name.
    pub name: String,
    /// Optional description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Network access mode.
    pub access: String,
    /// Network isolation mode.
    pub isolation: String,
}

/// Create network response.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct CreateNetworkResponse {
    /// Created network ID.
    pub id: String,
    /// Network name.
    pub name: String,
}

/// List networks response.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct ListNetworksResponse {
    /// List of networks.
    pub networks: Vec<NetworkInfo>,
    /// Total count.
    pub total: usize,
}

/// Network information (summary).
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct NetworkInfo {
    /// Network ID.
    pub id: String,
    /// Network name.
    pub name: String,
    /// Optional description.
    #[serde(default)]
    pub description: Option<String>,
    /// Network access mode.
    pub access: String,
    /// Number of members.
    pub member_count: usize,
    /// When the network was created.
    pub created_at: String,
}

/// Network detail (full information).
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct NetworkDetail {
    /// Network ID.
    pub id: String,
    /// Network name.
    pub name: String,
    /// Optional description.
    #[serde(default)]
    pub description: Option<String>,
    /// Network access mode.
    pub access: String,
    /// Network isolation mode.
    pub isolation: String,
    /// Network encryption mode.
    pub encryption: String,
    /// Network audit mode.
    pub audit: String,
    /// Member service IDs.
    #[serde(default)]
    pub members: Vec<String>,
    /// When the network was created.
    pub created_at: String,
    /// When the network was last updated.
    pub updated_at: String,
}

/// Join network request.
#[derive(Debug, serde::Serialize)]
pub struct JoinNetworkRequest {
    /// Service ID to add.
    pub service_id: String,
    /// Service name.
    pub service_name: String,
}

/// Leave network request.
#[derive(Debug, serde::Serialize)]
pub struct LeaveNetworkRequest {
    /// Service ID to remove.
    pub service_id: String,
}

// ==================== Volume Types ====================

/// Create volume request.
#[derive(Debug, serde::Serialize)]
pub struct CreateVolumeRequest {
    /// Volume name.
    pub name: String,
    /// Optional description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Optional size limit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
}

/// Create volume response.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct CreateVolumeResponse {
    /// Created volume ID.
    pub id: String,
    /// Volume name.
    pub name: String,
}

/// List volumes response.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct ListVolumesResponse {
    /// List of volumes.
    pub volumes: Vec<VolumeInfo>,
    /// Total count.
    pub total: usize,
}

/// Volume information (summary).
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct VolumeInfo {
    /// Volume ID.
    pub id: String,
    /// Volume name.
    pub name: String,
    /// Optional description.
    #[serde(default)]
    pub description: Option<String>,
    /// Optional size limit.
    #[serde(default)]
    pub size: Option<String>,
    /// Number of services using this volume.
    pub mount_count: usize,
    /// When the volume was created.
    pub created_at: String,
}

/// Volume detail (full information).
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct VolumeDetail {
    /// Volume ID.
    pub id: String,
    /// Volume name.
    pub name: String,
    /// Optional description.
    #[serde(default)]
    pub description: Option<String>,
    /// Optional size limit.
    #[serde(default)]
    pub size: Option<String>,
    /// Host path for the volume.
    pub path: String,
    /// Service IDs that have this volume mounted.
    #[serde(default)]
    pub mounted_by: Vec<String>,
    /// When the volume was created.
    pub created_at: String,
    /// When the volume was last updated.
    pub updated_at: String,
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

    // ==================== Service Operations ====================

    /// Lists all services.
    pub async fn list_services(&self) -> Result<ListServicesResponse> {
        self.get("/v1/services").await
    }

    /// Gets details about a specific service.
    pub async fn get_service(&self, id: &str) -> Result<ServiceDetail> {
        self.get(&format!("/v1/services/{id}")).await
    }

    /// Runs a Fabrickfile through the daemon.
    pub async fn run_fabrickfile(
        &self,
        req: RunFabrickfileRequest,
    ) -> Result<CreateServiceResponse> {
        self.post("/v1/services/run", &req).await
    }

    /// Runs a module by tag or registry reference through the daemon.
    pub async fn run_module(&self, req: RunModuleRequest) -> Result<CreateServiceResponse> {
        self.post("/v1/services/run-module", &req).await
    }

    /// Starts a service.
    pub async fn start_service(&self, id: &str) -> Result<()> {
        self.post_empty(&format!("/v1/services/{id}/start")).await
    }

    /// Stops a service.
    pub async fn stop_service(&self, id: &str) -> Result<()> {
        self.post_empty(&format!("/v1/services/{id}/stop")).await
    }

    /// Scales a service.
    pub async fn scale_service(&self, id: &str, replicas: usize) -> Result<()> {
        self.post(
            &format!("/v1/services/{id}/scale"),
            &ScaleServiceRequest { replicas },
        )
        .await
    }

    /// Deletes a service.
    pub async fn delete_service(&self, id: &str) -> Result<()> {
        self.delete(&format!("/v1/services/{id}")).await
    }

    // ==================== Mortar Operations ====================

    /// Deploys a mortar project.
    pub async fn deploy_mortar(&self, mortar_path: PathBuf) -> Result<DeployMortarResponse> {
        self.post("/v1/mortar/deploy", &DeployMortarRequest { mortar_path })
            .await
    }

    /// Lists all mortar projects.
    pub async fn list_projects(&self) -> Result<ListProjectsResponse> {
        self.get("/v1/mortar/projects").await
    }

    /// Gets services in a mortar project.
    pub async fn get_project_services(&self, name: &str) -> Result<ProjectServicesResponse> {
        self.get(&format!("/v1/mortar/projects/{name}")).await
    }

    /// Tears down a mortar project.
    pub async fn teardown_mortar(&self, name: &str) -> Result<()> {
        self.delete(&format!("/v1/mortar/projects/{name}")).await
    }

    // ==================== Network Operations ====================

    /// Creates a new network.
    pub async fn create_network(&self, req: CreateNetworkRequest) -> Result<CreateNetworkResponse> {
        self.post("/v1/networks", &req).await
    }

    /// Lists all networks.
    pub async fn list_networks(&self) -> Result<ListNetworksResponse> {
        self.get("/v1/networks").await
    }

    /// Gets details about a specific network.
    pub async fn get_network(&self, id: &str) -> Result<NetworkDetail> {
        self.get(&format!("/v1/networks/{id}")).await
    }

    /// Deletes a network.
    pub async fn delete_network(&self, id: &str) -> Result<()> {
        self.delete(&format!("/v1/networks/{id}")).await
    }

    /// Adds a service to a network.
    pub async fn join_network(&self, network_id: &str, req: JoinNetworkRequest) -> Result<()> {
        self.post(&format!("/v1/networks/{network_id}/join"), &req)
            .await
    }

    /// Removes a service from a network.
    pub async fn leave_network(&self, network_id: &str, req: LeaveNetworkRequest) -> Result<()> {
        self.post(&format!("/v1/networks/{network_id}/leave"), &req)
            .await
    }

    // ==================== Volume Operations ====================

    /// Creates a new volume.
    pub async fn create_volume(&self, req: CreateVolumeRequest) -> Result<CreateVolumeResponse> {
        self.post("/v1/volumes", &req).await
    }

    /// Lists all volumes.
    pub async fn list_volumes(&self) -> Result<ListVolumesResponse> {
        self.get("/v1/volumes").await
    }

    /// Gets details about a specific volume.
    pub async fn get_volume(&self, id: &str) -> Result<VolumeDetail> {
        self.get(&format!("/v1/volumes/{id}")).await
    }

    /// Deletes a volume.
    pub async fn delete_volume(&self, id: &str) -> Result<()> {
        self.delete(&format!("/v1/volumes/{id}")).await
    }

    // ==================== HTTP Methods ====================

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

    /// Performs a POST request with JSON body.
    async fn post<T: DeserializeOwned, B: Serialize>(&self, path: &str, body: &B) -> Result<T> {
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

        tokio::spawn(async move {
            if let Err(e) = conn.await {
                tracing::error!("Connection error: {e}");
            }
        });

        let json_body = serde_json::to_vec(body).context("Failed to serialize request")?;

        let request = Request::builder()
            .method("POST")
            .uri(path)
            .header("Host", "localhost")
            .header("Content-Type", "application/json")
            .body(Full::new(Bytes::from(json_body)))
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

    /// Performs a POST request without expecting response data.
    async fn post_empty(&self, path: &str) -> Result<()> {
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

        tokio::spawn(async move {
            if let Err(e) = conn.await {
                tracing::error!("Connection error: {e}");
            }
        });

        let request = Request::builder()
            .method("POST")
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

        let api_response: ApiResponse<()> =
            serde_json::from_slice(&body).context("Failed to parse response")?;

        match api_response {
            ApiResponse::Success { .. } => Ok(()),
            ApiResponse::Error { error } => {
                anyhow::bail!("{}: {}", error.code, error.message)
            }
        }
    }

    /// Performs a DELETE request.
    async fn delete(&self, path: &str) -> Result<()> {
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

        tokio::spawn(async move {
            if let Err(e) = conn.await {
                tracing::error!("Connection error: {e}");
            }
        });

        let request = Request::builder()
            .method("DELETE")
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

        let api_response: ApiResponse<()> =
            serde_json::from_slice(&body).context("Failed to parse response")?;

        match api_response {
            ApiResponse::Success { .. } => Ok(()),
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
