# Fabricks Daemon (fabricksd) Documentation

Complete reference for the Fabricks daemon and its REST API.

---

## Table of Contents

- [Overview](#overview)
- [Architecture](#architecture)
- [Installation](#installation)
- [Configuration](#configuration)
- [Starting & Stopping](#starting--stopping)
- [Socket API Reference](#socket-api-reference)
  - [Service Management](#service-management)
  - [Health & Monitoring](#health--monitoring)
  - [Networks](#networks)
  - [Volumes](#volumes)
  - [Mortar (Composition)](#mortar-composition)
  - [Daemon Management](#daemon-management)
  - [Events & Streaming](#events--streaming)
- [Authentication](#authentication)
- [Error Handling](#error-handling)
- [Metrics & Observability](#metrics--observability)
- [Security Model](#security-model)
- [Examples](#examples)

---

## Overview

**fabricksd** is the Fabricks daemon - a long-running background process that orchestrates WASM services, manages their lifecycle, and provides a REST API for control and monitoring.

### Why a Daemon?

The daemon is essential for:
- **Continuous health monitoring** - Check service health at intervals
- **Automatic restart** - Restart failed services based on policy
- **Auto-scaling** - Scale services based on resource usage
- **Service coordination** - Manage dependencies and startup order
- **Network management** - Create and enforce network segmentation
- **Policy enforcement** - Apply security and compliance policies
- **Log aggregation** - Collect and stream logs from all services
- **State persistence** - Track running services across CLI invocations

### Key Features

- **Unprivileged** - Runs as normal user (no root required)
- **Lightweight** - Minimal resource overhead
- **Socket-based** - Unix socket for local communication
- **REST API** - Standard HTTP/JSON interface
- **Event streaming** - Server-Sent Events (SSE) for real-time updates
- **Plugin-ready** - Extensible architecture

---

## Architecture

### Process Model
```
┌─────────────────────────────────────────────────────────────┐
│                       fabricksd                             │
│                                                             │
│  ┌────────────────┐  ┌──────────────┐  ┌────────────────┐   │
│  │ HTTP API       │  │ Health       │  │ Auto-Scaler    │   │
│  │ Server         │  │ Monitor      │  │                │   │
│  │ (Unix Socket)  │  │              │  │                │   │
│  └────────────────┘  └──────────────┘  └────────────────┘   │
│                                                             │
│  ┌────────────────┐  ┌──────────────┐  ┌────────────────┐   │
│  │ Service        │  │ Network      │  │ Volume         │   │
│  │ Manager        │  │ Manager      │  │ Manager        │   │
│  └────────────────┘  └──────────────┘  └────────────────┘   │
│                                                             │
│  ┌────────────────┐  ┌──────────────┐  ┌────────────────┐   │
│  │ Policy         │  │ Event        │  │ State          │   │
│  │ Engine         │  │ Bus          │  │ Store          │   │
│  └────────────────┘  └──────────────┘  └────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                            │
            ┌───────────────┼───────────────┐
            │               │               │
            ▼               ▼               ▼
    ┌──────────────┐ ┌──────────────┐ ┌──────────────┐
    │ Wasmtime     │ │ Wasmtime     │ │ Wasmtime     │
    │ Instance 1   │ │ Instance 2   │ │ Instance 3   │
    │              │ │              │ │              │
    │ service-a    │ │ service-a    │ │ service-b    │
    └──────────────┘ └──────────────┘ └──────────────┘
```

### Components

**API Server**
- Listens on Unix socket
- Handles HTTP requests
- Authentication & authorization
- Request routing

**Service Manager**
- Starts/stops WASM instances
- Manages replicas
- Handles restarts
- Tracks service state

**Health Monitor**
- Executes health checks
- Tracks service health history
- Triggers restart on failure

**Auto-Scaler**
- Monitors resource usage
- Scales services up/down
- Respects min/max replicas

**Network Manager**
- Creates network namespaces
- Enforces network policies
- Manages DNS resolution

**Volume Manager**
- Creates/mounts volumes
- Manages permissions
- Handles backups

**Policy Engine**
- Enforces security policies
- Validates operations
- Audits access

**Event Bus**
- Publishes events
- Streams to subscribers
- Event history

**State Store**
- Persists daemon state
- Service configurations
- Health history
- Metrics

---

## Installation

### From Binary
```bash
# Download latest release
curl -LO https://github.com/fabricks/fabricks/releases/latest/download/fabricksd-$(uname -s)-$(uname -m)
chmod +x fabricksd-*
sudo mv fabricksd-* /usr/local/bin/fabricksd

# Verify installation
fabricksd --version
```

### From Source
```bash
# Clone repository
git clone https://github.com/fabricks/fabricks.git
cd fabricks

# Build daemon
cargo build --release --bin fabricksd

# Install
sudo cp target/release/fabricksd /usr/local/bin/
```

### System Service (systemd)
```bash
# Create service file
sudo tee /etc/systemd/system/fabricksd.service <<EOF
[Unit]
Description=Fabricks Daemon
After=network.target

[Service]
Type=notify
ExecStart=/usr/local/bin/fabricksd start
Restart=on-failure
User=fabricks
Group=fabricks

[Install]
WantedBy=multi-user.target
EOF

# Enable and start
sudo systemctl enable fabricksd
sudo systemctl start fabricksd
```

### User Service (systemd)
```bash
# Create user service
mkdir -p ~/.config/systemd/user
tee ~/.config/systemd/user/fabricksd.service <<EOF
[Unit]
Description=Fabricks Daemon
After=network.target

[Service]
Type=notify
ExecStart=%h/.local/bin/fabricksd start
Restart=on-failure

[Install]
WantedBy=default.target
EOF

# Enable and start
systemctl --user enable fabricksd
systemctl --user start fabricksd
```

---

## Configuration

### Configuration File

**Location:** `/etc/fabricksd/config.toml` or `~/.fabricks/daemon.toml`
```toml
# Fabricks Daemon Configuration

[daemon]
# Unix socket path
socket = "/var/run/fabricks.sock"  # System-wide
# socket = "~/.fabricks/fabricks.sock"  # User-specific

# PID file
pid_file = "/var/run/fabricks.pid"

# Log level: trace, debug, info, warn, error
log_level = "info"

# Log output: stdout, stderr, file, syslog
log_output = "stdout"

# Log file (if log_output = "file")
log_file = "/var/log/fabricksd.log"

# Data directory
data_dir = "/var/lib/fabricksd"
# data_dir = "~/.local/share/fabricks"

[api]
# API version
version = "v1"

# Enable authentication
auth_enabled = false

# API key (if auth_enabled = true)
# api_key = "your-secret-key"

# Request timeout
timeout = "30s"

# Max request body size
max_body_size = "10MB"

[runtime]
# Default WASM runtime: wasmtime | wasmer
default_engine = "wasmtime"

# Runtime flags
engine_flags = ["--opt-level=2", "--cranelift-opt-level=speed"]

# Max parallel builds
max_parallel_builds = 4

# Build cache directory
build_cache_dir = "~/.fabricks/cache"

[resources]
# Global resource limits
max_memory_total = "8Gi"
max_cpu_total = 8.0

# Per-service defaults
default_memory_limit = "512Mi"
default_cpu_limit = 1.0

# Maximum services
max_services = 100

# Maximum replicas per service
max_replicas_per_service = 20

[monitoring]
# Health check interval
health_check_interval = "5s"

# Health check timeout
health_check_timeout = "3s"

# Metrics collection interval
metrics_interval = "10s"

# Metrics retention
metrics_retention = "24h"

# Enable Prometheus endpoint
prometheus_enabled = true
prometheus_port = 9090

[scaling]
# Auto-scaling evaluation interval
evaluation_interval = "30s"

# Cooldown period after scaling
scale_up_cooldown = "1m"
scale_down_cooldown = "5m"

# CPU thresholds
default_cpu_threshold = 70
cpu_stabilization_window = "2m"

[networking]
# Network driver: bridge | host
default_driver = "bridge"

# DNS server
dns_server = "8.8.8.8"

# Enable IPv6
enable_ipv6 = false

[storage]
# Volume driver: local | nfs
default_volume_driver = "local"

# Volume base path
volume_base_path = "/var/lib/fabricks/volumes"

# Enable volume encryption
volume_encryption = false

[security]
# Run services as specific user
default_user = "fabricks"

# Enable seccomp
enable_seccomp = true

# Enable AppArmor/SELinux
enable_mandatory_access_control = true

# Deny network by default
deny_network_by_default = false

[policy]
# Policy enforcement mode: enforce | audit | disabled
mode = "enforce"

# Policy files directory
policy_dir = "/etc/fabricksd/policies"

[events]
# Event buffer size
buffer_size = 1000

# Event retention
retention = "1h"

# Enable event persistence
persist_events = false
```

### Environment Variables

Override config with environment variables:
```bash
# Socket path
export FABRICKS_SOCKET=/tmp/fabricks.sock

# Log level
export FABRICKS_LOG_LEVEL=debug

# Data directory
export FABRICKS_DATA_DIR=~/.fabricks

# API key
export FABRICKS_API_KEY=secret-key
```

---

## Starting & Stopping

### Start Daemon
```bash
# Foreground (for debugging)
fabricksd start

# Background
fabricksd start --detach

# With custom config
fabricksd start --config /path/to/config.toml

# With specific socket
fabricksd start --socket /tmp/fabricks.sock
```

### Stop Daemon
```bash
# Graceful shutdown
fabricksd stop

# Force shutdown
fabricksd stop --force

# Stop with timeout
fabricksd stop --timeout 30s
```

### Status
```bash
# Check daemon status
fabricksd status

# Verbose status
fabricksd status --verbose
```

**Output:**
```
fabricksd is running
PID: 12345
Uptime: 2h 15m
Socket: /var/run/fabricks.sock
Services: 8 running, 0 stopped
Memory: 245 MB
CPU: 3.2%
```

### Logs
```bash
# View daemon logs
fabricksd logs

# Follow logs
fabricksd logs --follow

# Last N lines
fabricksd logs --tail 100

# Since timestamp
fabricksd logs --since 2025-01-15T10:00:00Z
```

### Reload Configuration
```bash
# Reload config without restart
fabricksd reload

# Validate config before reload
fabricksd reload --validate
```

---

## Socket API Reference

All API requests are made to the Unix socket at `/var/run/fabricks.sock` (or configured path).

### Base URL
```
unix:///var/run/fabricks.sock/v1
```

### Request Format
```http
GET /v1/services HTTP/1.1
Host: localhost
Content-Type: application/json
```

### Response Format

**Success:**
```json
{
  "status": "success",
  "data": { ... }
}
```

**Error:**
```json
{
  "status": "error",
  "error": {
    "code": "SERVICE_NOT_FOUND",
    "message": "Service 'api' not found",
    "details": { ... }
  }
}
```

---

## Service Management

### Create Service

**Endpoint:** `POST /v1/services`

**Request Body:**
```json
{
  "name": "api-service",
  "image": "wasm://my-api:v1.0.0",
  "networks": ["application"],
  "environment": {
    "DATABASE_URL": "postgres://localhost/mydb",
    "LOG_LEVEL": "info"
  },
  "ports": [
    {"host": 8080, "container": 8080}
  ],
  "volumes": [
    {"source": "api-data", "target": "/data"}
  ],
  "resources": {
    "memory": "512Mi",
    "cpu": 1.0
  },
  "replicas": {
    "min": 2,
    "max": 10
  },
  "health_check": {
    "type": "http",
    "path": "/health",
    "interval": "30s",
    "timeout": "5s",
    "retries": 3
  },
  "restart_policy": {
    "policy": "on-failure",
    "max_attempts": 3,
    "backoff": "10s"
  }
}
```

**Response:**
```json
{
  "status": "success",
  "data": {
    "id": "api-service-a1b2c3",
    "name": "api-service",
    "state": "starting",
    "replicas": 2,
    "created_at": "2025-01-15T10:23:45Z"
  }
}
```

**cURL Example:**
```bash
curl --unix-socket /var/run/fabricks.sock \
  -X POST \
  -H "Content-Type: application/json" \
  -d '{"name":"api-service","image":"wasm://my-api:v1.0.0"}' \
  http://localhost/v1/services
```

---

### Run Module

**Endpoint:** `POST /v1/services/run-module`

Runs a module from OCI storage by tag or reference. This is the primary endpoint for running services - it loads the module from local OCI storage, handles multi-layer modules (runtime + source layers for interpreted runtimes), creates a service, and starts it.

**Request Body:**
```json
{
  "reference": "rust-http:0.1.0",
  "args": [],
  "env_vars": [],
  "no_capabilities": false,
  "networks": ["default"]
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `reference` | string | Yes | Module tag (e.g., "my-module:1.0.0") |
| `args` | array | No | Command-line arguments to pass to the module |
| `env_vars` | array | No | Environment variable overrides as `[key, value]` tuples |
| `no_capabilities` | boolean | No | Disable capability enforcement (default: false) |
| `networks` | array | No | Array of network names to join (e.g., `["default", "application"]`). If omitted or empty, service is internal-only. Use `["default"]` for external access. |

**Response:**
```json
{
  "status": "success",
  "data": {
    "id": "svc-a1b2c3d4",
    "name": "rust-http"
  }
}
```

**cURL Example:**
```bash
curl --unix-socket /var/run/fabricks.sock \
  -X POST \
  -H "Content-Type: application/json" \
  -d '{"reference":"rust-http:0.1.0"}' \
  http://localhost/v1/services/run-module
```

**Notes:**
- For interpreted runtimes (Python, JavaScript), the module's runtime layer is used as the WASM binary and source layers are extracted to `/app`
- The module must exist in local OCI storage (use CLI to build and store first)

---

### List Services

**Endpoint:** `GET /v1/services`

**Query Parameters:**
- `state` - Filter by state (running, stopped, failed)
- `network` - Filter by network
- `label` - Filter by label (e.g., `tier=application`)

**Response:**
```json
{
  "status": "success",
  "data": {
    "services": [
      {
        "id": "api-service-a1b2c3",
        "name": "api-service",
        "state": "running",
        "replicas": {
          "desired": 2,
          "running": 2,
          "healthy": 2
        },
        "created_at": "2025-01-15T10:23:45Z",
        "updated_at": "2025-01-15T10:25:30Z"
      },
      {
        "id": "worker-b4c5d6",
        "name": "worker",
        "state": "running",
        "replicas": {
          "desired": 5,
          "running": 5,
          "healthy": 4
        },
        "created_at": "2025-01-15T10:24:00Z",
        "updated_at": "2025-01-15T11:00:00Z"
      }
    ],
    "total": 2
  }
}
```

**cURL Example:**
```bash
curl --unix-socket /var/run/fabricks.sock \
  http://localhost/v1/services?state=running
```

---

### Get Service

**Endpoint:** `GET /v1/services/{id}`

**Response:**
```json
{
  "status": "success",
  "data": {
    "id": "api-service-a1b2c3",
    "name": "api-service",
    "image": "wasm://my-api:v1.0.0",
    "state": "running",
    "networks": ["application"],
    "environment": {
      "DATABASE_URL": "postgres://localhost/mydb",
      "LOG_LEVEL": "info"
    },
    "ports": [
      {"host": 8080, "container": 8080}
    ],
    "replicas": {
      "desired": 2,
      "running": 2,
      "healthy": 2,
      "instances": [
        {
          "id": "api-service-a1b2c3-0",
          "state": "running",
          "health": "healthy",
          "started_at": "2025-01-15T10:23:45Z",
          "restarts": 0
        },
        {
          "id": "api-service-a1b2c3-1",
          "state": "running",
          "health": "healthy",
          "started_at": "2025-01-15T10:23:46Z",
          "restarts": 0
        }
      ]
    },
    "resources": {
      "memory": {
        "limit": "512Mi",
        "usage": "245Mi"
      },
      "cpu": {
        "limit": 1.0,
        "usage": 0.35
      }
    },
    "created_at": "2025-01-15T10:23:45Z",
    "updated_at": "2025-01-15T10:25:30Z"
  }
}
```

---

### Update Service

**Endpoint:** `PATCH /v1/services/{id}`

**Request Body:**
```json
{
  "environment": {
    "LOG_LEVEL": "debug"
  },
  "replicas": {
    "min": 3,
    "max": 15
  }
}
```

**Response:**
```json
{
  "status": "success",
  "data": {
    "id": "api-service-a1b2c3",
    "updated_at": "2025-01-15T11:30:00Z"
  }
}
```

---

### Delete Service

**Endpoint:** `DELETE /v1/services/{id}`

**Query Parameters:**
- `force` - Force delete (default: false)
- `timeout` - Graceful shutdown timeout (default: 10s)

**Response:**
```json
{
  "status": "success",
  "data": {
    "id": "api-service-a1b2c3",
    "deleted_at": "2025-01-15T12:00:00Z"
  }
}
```

---

### Restart Service

**Endpoint:** `POST /v1/services/{id}/restart`

**Request Body:**
```json
{
  "timeout": "30s",
  "force": false
}
```

**Response:**
```json
{
  "status": "success",
  "data": {
    "id": "api-service-a1b2c3",
    "restarted_at": "2025-01-15T12:15:00Z",
    "instances_restarted": 2
  }
}
```

---

### Scale Service

**Endpoint:** `POST /v1/services/{id}/scale`

**Request Body:**
```json
{
  "replicas": 5
}
```

**Response:**
```json
{
  "status": "success",
  "data": {
    "id": "api-service-a1b2c3",
    "previous_replicas": 2,
    "new_replicas": 5,
    "scaled_at": "2025-01-15T12:20:00Z"
  }
}
```

---

### Get Service Logs

**Endpoint:** `GET /v1/services/:id/logs`

Retrieves logs for a service. The `:id` parameter can be either a service ID or service name.

**Query Parameters:**
- `tail` - Number of lines from end (default: all, 0 means all)

**Log Storage:**

Service stdout and stderr are captured by the daemon in a **per-service bounded ring buffer** (default: 10,000 lines). Logs are stored in memory with automatic rotation when the buffer is full. Each log entry includes:
- Timestamp (when the log was captured)
- Stream (`"stdout"` or `"stderr"`)
- Message (the actual log line)

**Response:**
```json
{
  "status": "success",
  "data": {
    "id": "srv_abc123",
    "entries": [
      {
        "timestamp": "2025-01-15T10:23:45.123456789Z",
        "stream": "stdout",
        "message": "Starting server on :8080"
      },
      {
        "timestamp": "2025-01-15T10:23:45.234567890Z",
        "stream": "stdout",
        "message": "Connected to database"
      },
      {
        "timestamp": "2025-01-15T10:24:00.345678901Z",
        "stream": "stderr",
        "message": "Warning: deprecated API usage"
      }
    ],
    "count": 3
  }
}
```

**Response Fields:**
- `id` - Service ID
- `entries` - Array of log entries
  - `timestamp` - RFC3339 timestamp with nanosecond precision
  - `stream` - Either `"stdout"` or `"stderr"`
  - `message` - Log message text
- `count` - Number of log entries returned

**cURL Examples:**
```bash
# Get all logs by service name
curl --unix-socket /var/run/fabricks.sock \
  "http://localhost/v1/services/my-service/logs"

# Get all logs by service ID
curl --unix-socket /var/run/fabricks.sock \
  "http://localhost/v1/services/srv_abc123/logs"

# Get last 100 lines
curl --unix-socket /var/run/fabricks.sock \
  "http://localhost/v1/services/my-service/logs?tail=100"

# Get all logs (explicit)
curl --unix-socket /var/run/fabricks.sock \
  "http://localhost/v1/services/my-service/logs?tail=0"
```

**Notes:**
- If service is not found, returns 404
- If service has no logs yet, returns empty `entries` array with `count: 0`
- Logs are per-service (not per-instance) - all instances write to the same log buffer
- Future: streaming with SSE, filtering by instance, time-based filtering

---

### Get Service Stats

**Endpoint:** `GET /v1/services/{id}/stats`

**Query Parameters:**
- `stream` - Stream stats (default: false)

**Response:**
```json
{
  "status": "success",
  "data": {
    "id": "api-service-a1b2c3",
    "stats": {
      "cpu": {
        "usage": 0.35,
        "limit": 1.0,
        "percentage": 35.0
      },
      "memory": {
        "usage": 245760000,
        "limit": 536870912,
        "percentage": 45.8
      },
      "network": {
        "rx_bytes": 1024000,
        "tx_bytes": 2048000
      },
      "requests": {
        "total": 1523,
        "rate": 12.5
      }
    },
    "timestamp": "2025-01-15T12:30:00Z"
  }
}
```

---

## Health & Monitoring

### Get Service Health

**Endpoint:** `GET /v1/health/{id}`

**Response:**
```json
{
  "status": "success",
  "data": {
    "id": "api-service-a1b2c3",
    "health": "healthy",
    "instances": [
      {
        "id": "api-service-a1b2c3-0",
        "health": "healthy",
        "last_check": "2025-01-15T12:30:00Z",
        "consecutive_failures": 0
      },
      {
        "id": "api-service-a1b2c3-1",
        "health": "healthy",
        "last_check": "2025-01-15T12:30:01Z",
        "consecutive_failures": 0
      }
    ]
  }
}
```

---

### Get Health History

**Endpoint:** `GET /v1/health/{id}/history`

**Query Parameters:**
- `since` - Start timestamp
- `until` - End timestamp
- `limit` - Max results (default: 100)

**Response:**
```json
{
  "status": "success",
  "data": {
    "id": "api-service-a1b2c3",
    "history": [
      {
        "timestamp": "2025-01-15T12:30:00Z",
        "instance": "api-service-a1b2c3-0",
        "status": "healthy",
        "response_time": "15ms"
      },
      {
        "timestamp": "2025-01-15T12:29:30Z",
        "instance": "api-service-a1b2c3-0",
        "status": "healthy",
        "response_time": "12ms"
      }
    ]
  }
}
```

---

### Trigger Health Check

**Endpoint:** `POST /v1/health/{id}/check`

**Response:**
```json
{
  "status": "success",
  "data": {
    "id": "api-service-a1b2c3",
    "health": "healthy",
    "checked_at": "2025-01-15T12:35:00Z"
  }
}
```

---

## Networks

### Create Network

**Endpoint:** `POST /v1/networks`

**Request Body:**
```json
{
  "name": "application",
  "internal": true,
  "ingress": ["dmz"],
  "egress": ["data", "cache"],
  "encryption": "required"
}
```

**Response:**
```json
{
  "status": "success",
  "data": {
    "id": "network-x1y2z3",
    "name": "application",
    "created_at": "2025-01-15T10:00:00Z"
  }
}
```

---

### List Networks

**Endpoint:** `GET /v1/networks`

**Response:**
```json
{
  "status": "success",
  "data": {
    "networks": [
      {
        "id": "network-x1y2z3",
        "name": "application",
        "internal": true,
        "services": ["api-service", "user-service"],
        "created_at": "2025-01-15T10:00:00Z"
      }
    ]
  }
}
```

---

### Get Network

**Endpoint:** `GET /v1/networks/{id}`

**Response:**
```json
{
  "status": "success",
  "data": {
    "id": "network-x1y2z3",
    "name": "application",
    "internal": true,
    "ingress": ["dmz"],
    "egress": ["data", "cache"],
    "services": [
      {
        "id": "api-service-a1b2c3",
        "name": "api-service"
      }
    ],
    "created_at": "2025-01-15T10:00:00Z"
  }
}
```

---

### Delete Network

**Endpoint:** `DELETE /v1/networks/{id}`

**Response:**
```json
{
  "status": "success",
  "data": {
    "id": "network-x1y2z3",
    "deleted_at": "2025-01-15T13:00:00Z"
  }
}
```

---

## Volumes

### Create Volume

**Endpoint:** `POST /v1/volumes`

**Request Body:**
```json
{
  "name": "postgres-data",
  "size": "50Gi",
  "encrypted": true,
  "backup": {
    "enabled": true,
    "schedule": "0 3 * * *",
    "retention": "30d"
  }
}
```

**Response:**
```json
{
  "status": "success",
  "data": {
    "id": "volume-p1q2r3",
    "name": "postgres-data",
    "size": "50Gi",
    "created_at": "2025-01-15T10:00:00Z"
  }
}
```

---

### List Volumes

**Endpoint:** `GET /v1/volumes`

**Response:**
```json
{
  "status": "success",
  "data": {
    "volumes": [
      {
        "id": "volume-p1q2r3",
        "name": "postgres-data",
        "size": "50Gi",
        "used": "12.3Gi",
        "mount_count": 1,
        "created_at": "2025-01-15T10:00:00Z"
      }
    ]
  }
}
```

---

### Get Volume

**Endpoint:** `GET /v1/volumes/{id}`

**Response:**
```json
{
  "status": "success",
  "data": {
    "id": "volume-p1q2r3",
    "name": "postgres-data",
    "size": "50Gi",
    "used": "12.3Gi",
    "encrypted": true,
    "mounted_by": [
      {
        "service_id": "postgres-s4t5u6",
        "mount_point": "/var/lib/postgresql/data"
      }
    ],
    "backups": {
      "enabled": true,
      "last_backup": "2025-01-15T03:00:00Z",
      "next_backup": "2025-01-16T03:00:00Z"
    },
    "created_at": "2025-01-15T10:00:00Z"
  }
}
```

---

### Delete Volume

**Endpoint:** `DELETE /v1/volumes/{id}`

**Query Parameters:**
- `force` - Force delete even if in use (default: false)

**Response:**
```json
{
  "status": "success",
  "data": {
    "id": "volume-p1q2r3",
    "deleted_at": "2025-01-15T14:00:00Z"
  }
}
```

---

## Mortar (Composition)

### Deploy Mortar

**Endpoint:** `POST /v1/mortar/deploy`

**Request Body:**
```json
{
  "mortar": "...",  // fabricks-mortar.toml content (base64 encoded)
  "profile": "production",
  "build": true,
  "force_recreate": false
}
```

**Response:**
```json
{
  "status": "success",
  "data": {
    "deployment_id": "deploy-v1w2x3",
    "services_created": 8,
    "networks_created": 4,
    "volumes_created": 3,
    "deployed_at": "2025-01-15T10:00:00Z"
  }
}
```

---

### Undeploy Mortar

**Endpoint:** `POST /v1/mortar/undeploy`

**Request Body:**
```json
{
  "project_name": "acme-shop",
  "remove_volumes": false,
  "timeout": "30s"
}
```

**Response:**
```json
{
  "status": "success",
  "data": {
    "services_removed": 8,
    "networks_removed": 4,
    "volumes_removed": 0,
    "undeployed_at": "2025-01-15T15:00:00Z"
  }
}
```

---

### Get Mortar Status

**Endpoint:** `GET /v1/mortar/status/{project_name}`

**Response:**
```json
{
  "status": "success",
  "data": {
    "project_name": "acme-shop",
    "state": "running",
    "services": {
      "total": 8,
      "running": 8,
      "failed": 0
    },
    "health": "healthy",
    "deployed_at": "2025-01-15T10:00:00Z",
    "updated_at": "2025-01-15T12:00:00Z"
  }
}
```

---

## Daemon Management

### Get Daemon Info

**Endpoint:** `GET /v1/daemon/info`

**Response:**
```json
{
  "status": "success",
  "data": {
    "version": "1.0.0",
    "api_version": "v1",
    "runtime": "wasmtime 25.0.0",
    "platform": "linux/amd64",
    "started_at": "2025-01-15T08:00:00Z",
    "uptime": "4h 30m 15s",
    "config": {
      "socket": "/var/run/fabricks.sock",
      "data_dir": "/var/lib/fabricksd",
      "max_services": 100
    }
  }
}
```

---

### Get Daemon Stats

**Endpoint:** `GET /v1/daemon/stats`

**Response:**
```json
{
  "status": "success",
  "data": {
    "services": {
      "total": 8,
      "running": 8,
      "stopped": 0,
      "failed": 0
    },
    "networks": {
      "total": 4
    },
    "volumes": {
      "total": 3,
      "size": "65.3Gi"
    },
    "resources": {
      "memory": {
        "used": "2.1Gi",
        "limit": "8Gi"
      },
      "cpu": {
        "used": 3.2,
        "limit": 8.0
      }
    },
    "api": {
      "requests_total": 15234,
      "requests_per_second": 12.5,
      "errors_total": 23
    }
  }
}
```

---

### Reload Daemon Config

**Endpoint:** `POST /v1/daemon/reload`

**Response:**
```json
{
  "status": "success",
  "data": {
    "reloaded_at": "2025-01-15T13:00:00Z",
    "changes": [
      "log_level: info -> debug",
      "max_services: 100 -> 200"
    ]
  }
}
```

---

### Shutdown Daemon

**Endpoint:** `POST /v1/daemon/shutdown`

**Request Body:**
```json
{
  "timeout": "30s",
  "force": false
}
```

**Response:**
```json
{
  "status": "success",
  "data": {
    "message": "Daemon shutting down gracefully",
    "services_to_stop": 8
  }
}
```

---

## Events & Streaming

### Subscribe to Events

**Endpoint:** `GET /v1/events`

**Query Parameters:**
- `types` - Filter event types (comma-separated)
- `service` - Filter by service ID
- `since` - Only events after timestamp

**Event Types:**
- `service.created`
- `service.started`
- `service.stopped`
- `service.failed`
- `service.scaled`
- `health.changed`
- `network.created`
- `volume.created`
- `policy.violated`

**Response (SSE Stream):**
```
event: service.started
data: {"service_id":"api-service-a1b2c3","instance":"api-service-a1b2c3-0","timestamp":"2025-01-15T10:23:45Z"}

event: health.changed
data: {"service_id":"api-service-a1b2c3","instance":"api-service-a1b2c3-0","health":"healthy","timestamp":"2025-01-15T10:24:00Z"}

event: service.scaled
data: {"service_id":"api-service-a1b2c3","previous_replicas":2,"new_replicas":5,"timestamp":"2025-01-15T12:20:00Z"}
```

**cURL Example:**
```bash
curl --unix-socket /var/run/fabricks.sock \
  -N \
  "http://localhost/v1/events?types=service.started,service.failed"
```

---

### Get Event History

**Endpoint:** `GET /v1/events/history`

**Query Parameters:**
- `types` - Filter event types
- `service` - Filter by service ID
- `since` - Start timestamp
- `until` - End timestamp
- `limit` - Max results (default: 100)

**Response:**
```json
{
  "status": "success",
  "data": {
    "events": [
      {
        "id": "event-e1f2g3",
        "type": "service.started",
        "service_id": "api-service-a1b2c3",
        "data": {
          "instance": "api-service-a1b2c3-0"
        },
        "timestamp": "2025-01-15T10:23:45Z"
      }
    ],
    "total": 1523
  }
}
```

---

## Authentication

### API Key Authentication

When `auth_enabled = true` in config, all requests must include API key:

**Header:**
```
X-Fabricks-API-Key: your-secret-key
```

**cURL Example:**
```bash
curl --unix-socket /var/run/fabricks.sock \
  -H "X-Fabricks-API-Key: your-secret-key" \
  http://localhost/v1/services
```

### Generate API Key
```bash
# Generate new API key
fabricksd api-key generate

# List API keys
fabricksd api-key list

# Revoke API key
fabricksd api-key revoke <key-id>
```

---

## Error Handling

### Error Response Format
```json
{
  "status": "error",
  "error": {
    "code": "SERVICE_NOT_FOUND",
    "message": "Service 'api-service' not found",
    "details": {
      "service_id": "api-service",
      "available_services": ["worker", "postgres"]
    }
  }
}
```

### Error Codes

| Code | HTTP Status | Description |
|------|-------------|-------------|
| `INVALID_REQUEST` | 400 | Malformed request |
| `UNAUTHORIZED` | 401 | Missing/invalid API key |
| `FORBIDDEN` | 403 | Insufficient permissions |
| `SERVICE_NOT_FOUND` | 404 | Service doesn't exist |
| `NETWORK_NOT_FOUND` | 404 | Network doesn't exist |
| `VOLUME_NOT_FOUND` | 404 | Volume doesn't exist |
| `SERVICE_ALREADY_EXISTS` | 409 | Service name conflict |
| `INVALID_STATE` | 409 | Operation not allowed in current state |
| `RESOURCE_LIMIT_EXCEEDED` | 429 | Too many services/requests |
| `BUILD_FAILED` | 500 | Service build failed |
| `RUNTIME_ERROR` | 500 | WASM runtime error |
| `INTERNAL_ERROR` | 500 | Daemon internal error |

---

## Metrics & Observability

### Prometheus Metrics

**Endpoint:** `http://localhost:9090/metrics` (if enabled)

**Metrics:**
```prometheus
# Service metrics
fabricks_services_total{state="running"} 8
fabricks_services_total{state="stopped"} 0
fabricks_services_total{state="failed"} 0

# Resource metrics
fabricks_memory_used_bytes 2251799814
fabricks_memory_limit_bytes 8589934592
fabricks_cpu_used_cores 3.2
fabricks_cpu_limit_cores 8.0

# Health metrics
fabricks_health_checks_total{service="api-service",result="success"} 1523
fabricks_health_checks_total{service="api-service",result="failure"} 2

# API metrics
fabricks_api_requests_total{method="GET",endpoint="/services"} 523
fabricks_api_requests_total{method="POST",endpoint="/services"} 45
fabricks_api_request_duration_seconds{endpoint="/services"} 0.015

# Scaling metrics
fabricks_service_replicas{service="api-service"} 5
fabricks_autoscale_events_total{service="api-service",direction="up"} 3
```

---

## Security Model

### Socket Permissions
```bash
# Socket owned by fabricks group
srw-rw---- 1 fabricks fabricks 0 Jan 15 10:00 /var/run/fabricks.sock

# Add user to fabricks group
sudo usermod -aG fabricks $USER
```

### Unprivileged Operation

- Runs as normal user (no root required)
- Services run as configured user (default: `fabricks`)
- Capabilities denied by default
- Network policies enforced
- Volume permissions managed

### Policy Enforcement
```toml
# Example policy
[policy.pci_compliance]

[[policy.pci_compliance.deny]]
from = ["payment"]
to = ["monitoring"]
reason = "PCI data must not be logged"
```

**API validates policies before execution**

---

## Examples

### Create and Monitor Service (Bash)
```bash
#!/bin/bash

SOCKET="/var/run/fabricks.sock"
API="http://localhost/v1"

# Create service
SERVICE_ID=$(curl -s --unix-socket $SOCKET \
  -X POST \
  -H "Content-Type: application/json" \
  -d '{
    "name": "my-api",
    "image": "wasm://my-api:v1.0.0",
    "networks": ["application"],
    "replicas": {"min": 2, "max": 10}
  }' \
  "$API/services" | jq -r '.data.id')

echo "Created service: $SERVICE_ID"

# Wait for healthy
while true; do
  HEALTH=$(curl -s --unix-socket $SOCKET \
    "$API/health/$SERVICE_ID" | jq -r '.data.health')
  
  if [ "$HEALTH" == "healthy" ]; then
    echo "Service is healthy!"
    break
  fi
  
  echo "Waiting for healthy... (current: $HEALTH)"
  sleep 5
done

# Stream logs
curl -s --unix-socket $SOCKET \
  -N \
  "$API/services/$SERVICE_ID/logs?follow=true"
```

### Monitor Events (Python)
```python
import requests_unixsocket
import json

session = requests_unixsocket.Session()
url = 'http+unix://%2Fvar%2Frun%2Ffabricks.sock/v1/events'

response = session.get(url, stream=True)

for line in response.iter_lines():
    if line:
        line = line.decode('utf-8')
        if line.startswith('data: '):
            event_data = json.loads(line[6:])
            print(f"Event: {event_data}")
```

### Auto-scale Based on Custom Metric (JavaScript/Node.js)
```javascript
const axios = require('axios');
const http = require('http');

const agent = new http.Agent({
  socketPath: '/var/run/fabricks.sock'
});

const api = axios.create({
  baseURL: 'http://localhost/v1',
  httpAgent: agent
});

async function autoScale() {
  // Get service stats
  const { data } = await api.get('/services/api-service-a1b2c3/stats');
  
  const cpuUsage = data.data.stats.cpu.percentage;
  const currentReplicas = data.data.replicas.running;
  
  if (cpuUsage > 80 && currentReplicas < 10) {
    // Scale up
    await api.post('/services/api-service-a1b2c3/scale', {
      replicas: currentReplicas + 2
    });
    console.log(`Scaled up to ${currentReplicas + 2} replicas`);
  } else if (cpuUsage < 30 && currentReplicas > 2) {
    // Scale down
    await api.post('/services/api-service-a1b2c3/scale', {
      replicas: currentReplicas - 1
    });
    console.log(`Scaled down to ${currentReplicas - 1} replicas`);
  }
}

// Check every 30 seconds
setInterval(autoScale, 30000);
```

---
