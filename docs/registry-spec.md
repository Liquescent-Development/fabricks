# Fabricks Registry Specification

Complete specification for Fabricks registry storage, transfer, and distribution using OCI standards.

---

## Table of Contents

- [Overview](#overview)
- [Why OCI?](#why-oci)
- [Media Types](#media-types)
- [OCI Manifest Structure](#oci-manifest-structure)
- [Local Storage Format](#local-storage-format)
- [Transfer Protocol](#transfer-protocol)
- [Authentication](#authentication)
- [Registry URL Format](#registry-url-format)
- [Metadata and Annotations](#metadata-and-annotations)
- [Content Verification](#content-verification)
- [Registry Configuration](#registry-configuration)
- [Supported Registries](#supported-registries)
- [Examples](#examples)
- [Compatibility Matrix](#compatibility-matrix)

---

## Overview

Fabricks uses the **OCI Distribution Specification** for storing and distributing WASM modules. This means Fabricks works with any OCI-compliant registry including Docker Hub, GitHub Container Registry, Google GCR, AWS ECR, Harbor, and more.

**Key Design Decisions:**
- Use **OCI Artifacts** (not OCI Container Images)
- Custom media types for WASM modules and Fabrickfiles
- Content-addressable storage by SHA256 digest
- Docker-compatible authentication
- Standard OCI Distribution API for push/pull

---

## Why OCI?

### Benefits

1. **Existing Infrastructure**
   - Works with Docker Hub, GHCR, ECR, GCR, ACR, Harbor, Artifactory
   - No new registry infrastructure needed
   - Leverage existing registry mirrors and caching

2. **Well Tested Protocol**
   - OCI Distribution Spec proven at scale
   - Handles multi-gigabyte artifacts efficiently
   - Built-in resumable uploads/downloads
   - Chunked transfer support

3. **Authentication**
   - Standard bearer token authentication
   - `docker login` credentials work with Fabricks
   - Integration with existing identity providers

4. **Content-addressable Storage**
   - SHA256 digests ensure integrity
   - Automatic deduplication
   - Immutable references via digest

5. **Ecosystem Tooling**
   - Registry replication and mirroring
   - Vulnerability scanning
   - SBOM and signature support (Cosign, Notary)
   - Air-gapped deployments

### Precedent

Other projects using OCI for WASM:
- **SpinKube** - WASM apps in Kubernetes via OCI
- **wasmCloud** - Actor model WASM platform
- **Docker+Wasm** - Docker's WASM runtime integration
- **ORAS** - OCI Registry As Storage (generic artifacts)

---

## Media Types

Fabricks defines custom OCI media types for WASM artifacts:

### Core Media Types
```
WASM Module:
application/vnd.fabricks.module.v1+wasm

Fabrickfile Config:
application/vnd.fabricks.config.v1+toml

Metadata:
application/vnd.fabricks.metadata.v1+json

Component Model Interface:
application/vnd.fabricks.component.v1+wasm

Signature (Cosign):
application/vnd.dev.cosign.signature.v1+json
```

### Media Type Usage

| Type | Purpose | Stored As | Required |
|------|---------|-----------|----------|
| `module.v1+wasm` | Compiled WASM binary | Layer | Yes |
| `config.v1+toml` | Fabrickfile source | Config blob | Yes |
| `metadata.v1+json` | Build/runtime metadata | Annotation | No |
| `component.v1+wasm` | Component Model interface | Layer | No |
| `signature.v1+json` | Cosign signature | Separate artifact | No |

---

## OCI Manifest Structure

### Complete Manifest Example
```json
{
  "schemaVersion": 2,
  "mediaType": "application/vnd.oci.image.manifest.v1+json",
  "artifactType": "application/vnd.fabricks.module.v1",
  "config": {
    "mediaType": "application/vnd.fabricks.config.v1+toml",
    "digest": "sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a",
    "size": 2048,
    "annotations": {
      "org.opencontainers.image.title": "Fabrickfile"
    }
  },
  "layers": [
    {
      "mediaType": "application/vnd.fabricks.module.v1+wasm",
      "digest": "sha256:9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08",
      "size": 2457600,
      "annotations": {
        "org.opencontainers.image.title": "product-service.wasm"
      }
    }
  ],
  "annotations": {
    "org.opencontainers.image.created": "2025-01-15T10:23:45Z",
    "org.opencontainers.image.authors": "backend-team@acme.com",
    "org.opencontainers.image.description": "Product catalog service",
    "org.opencontainers.image.version": "2.1.0",
    "org.opencontainers.image.licenses": "MIT",
    "org.opencontainers.image.source": "https://github.com/acme/product-service",
    "org.opencontainers.image.documentation": "https://docs.acme.com/services/product",
    "org.opencontainers.image.revision": "abc123def456",
    "dev.fabricks.exports": "[\"list_products\",\"get_product\",\"create_product\"]",
    "dev.fabricks.imports": "{\"search_indexer\":\"wasm://search/indexer:v1.0\"}",
    "dev.fabricks.capabilities": "{\"network\":{\"listen\":[8080]},\"env\":[\"DATABASE_URL\"]}",
    "dev.fabricks.platform": "wasm32-wasi",
    "dev.fabricks.runtime": "wasmtime:25.0.0"
  }
}
```

### Manifest Components

#### Config Blob

The config blob contains the original Fabrickfile as TOML:

**Blob digest:** `sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a`

**Content:**
```toml
fabrick_version = "1.0"

[info]
name = "product-service"
version = "2.1.0"
description = "Product catalog service for e-commerce platform"
authors = ["backend-team@acme.com"]
license = "MIT"

[from]
source = "rust"

[source]
path = "."
include = ["src/**/*.rs", "Cargo.toml"]

[build]
command = "cargo build --target wasm32-wasi --release"
output = "target/wasm32-wasi/release/product_service.wasm"

exports = ["list_products", "get_product", "create_product"]

[imports]
search_indexer = "wasm://search/indexer:v1.0"

[capabilities]
env = ["DATABASE_URL", "LOG_LEVEL"]

[capabilities.network]
listen = [8080]
connect = ["postgres:5432", "redis:6379"]

[health_check.http]
path = "/health"
interval = "30s"
```

#### Layer (WASM Module)

**Blob digest:** `sha256:9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08`

**Media type:** `application/vnd.fabricks.module.v1+wasm`

**Size:** 2,457,600 bytes (2.4 MB)

**Content:** Raw WASM binary bytes

---

## Local Storage Format

Fabricks follows the **OCI Image Layout Specification** for local storage.

### Directory Structure
```
~/.fabricks/
├── registry/                        # OCI layout root
│   ├── blobs/
│   │   └── sha256/
│   │       ├── 9f86d081884c7d65...  # WASM module (2.4 MB)
│   │       ├── 44136fa355b3678a...  # Fabrickfile config (2 KB)
│   │       ├── a1b2c3d4e5f6a7b8...  # Manifest (1 KB)
│   │       ├── b2c3d4e5f6a7b8c9...  # Another WASM module
│   │       └── c3d4e5f6a7b8c9d0...  # Another config
│   │
│   ├── manifests/
│   │   ├── registry.acme.io/
│   │   │   ├── product-service/
│   │   │   │   ├── v2.1.0           # → sha256:a1b2c3d4e5f6a7b8...
│   │   │   │   ├── v2.0.0           # → sha256:...
│   │   │   │   └── latest           # → sha256:a1b2c3d4e5f6a7b8...
│   │   │   └── user-service/
│   │   │       └── v1.0.0
│   │   │
│   │   ├── ghcr.io/
│   │   │   └── acme/
│   │   │       └── cart-service/
│   │   │           └── v1.5.0
│   │   │
│   │   └── registry.fabricks.io/
│   │       └── library/
│   │           ├── redis/
│   │           │   └── 7.2
│   │           └── postgres/
│   │               └── 16
│   │
│   ├── index.json                   # OCI Layout index
│   └── oci-layout                   # Version marker
│
├── config.toml                      # Fabricks configuration
├── credentials.json                 # Registry authentication
└── cache/                           # Build cache
    └── ...
```

### OCI Layout Files

#### `oci-layout`

Version marker for OCI Image Layout:
```json
{
  "imageLayoutVersion": "1.0.0"
}
```

#### `index.json`

Index of all locally stored images:
```json
{
  "schemaVersion": 2,
  "manifests": [
    {
      "mediaType": "application/vnd.oci.image.manifest.v1+json",
      "digest": "sha256:a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2",
      "size": 1024,
      "annotations": {
        "org.opencontainers.image.ref.name": "registry.acme.io/product-service:v2.1.0",
        "dev.fabricks.name": "product-service",
        "dev.fabricks.version": "v2.1.0"
      }
    },
    {
      "mediaType": "application/vnd.oci.image.manifest.v1+json",
      "digest": "sha256:b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3",
      "size": 1156,
      "annotations": {
        "org.opencontainers.image.ref.name": "ghcr.io/acme/cart-service:v1.5.0",
        "dev.fabricks.name": "cart-service",
        "dev.fabricks.version": "v1.5.0"
      }
    }
  ]
}
```

### Blob Storage

All content is stored content-addressably in `blobs/sha256/`:
```
blobs/sha256/9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08
```

**Benefits:**
- **Deduplication** - Same content stored once, even across different images
- **Integrity** - Content verified by SHA256 on read
- **Immutability** - Digest changes if content changes
- **Efficiency** - Sharing common layers between fabricks

**Example Deduplication:**

If `product-service:v2.1.0` and `product-service:v2.0.0` have the same Fabrickfile, the config blob is stored once and referenced twice.

### Manifest References

Tag-to-digest mappings stored in `manifests/`:
```
manifests/registry.acme.io/product-service/v2.1.0
```

**Content:**
```
sha256:a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2
```

This allows:
- Quick tag resolution without parsing JSON
- Atomic tag updates (write new file)
- Tag aliasing via symlinks

---

## Transfer Protocol

Fabricks uses the **OCI Distribution Specification** for push/pull operations.

### Pull Flow

**Command:**
```bash
fabricks pull registry.acme.io/product-service:v2.1.0
```

#### Step 1: Authenticate
```http
GET /v2/
Host: registry.acme.io
Authorization: Bearer <token>

Response:
200 OK
Docker-Distribution-API-Version: registry/2.0
```

#### Step 2: Fetch Manifest
```http
GET /v2/product-service/manifests/v2.1.0
Host: registry.acme.io
Accept: application/vnd.oci.image.manifest.v1+json
Authorization: Bearer <token>

Response:
200 OK
Content-Type: application/vnd.oci.image.manifest.v1+json
Docker-Content-Digest: sha256:a1b2c3d4e5f6a7b8...

{
  "schemaVersion": 2,
  "config": {
    "digest": "sha256:44136fa355b3678a...",
    "size": 2048
  },
  "layers": [
    {
      "digest": "sha256:9f86d081884c7d65...",
      "size": 2457600
    }
  ]
}
```

#### Step 3: Check Local Cache

Check if blobs exist locally:
```bash
# Check config blob
ls ~/.fabricks/registry/blobs/sha256/44136fa355b3678a...

# Check WASM module
ls ~/.fabricks/registry/blobs/sha256/9f86d081884c7d65...
```

If exists, skip download (content-addressable win!).

#### Step 4: Fetch Config Blob
```http
GET /v2/product-service/blobs/sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a
Host: registry.acme.io
Authorization: Bearer <token>

Response:
200 OK
Content-Type: application/vnd.fabricks.config.v1+toml
Content-Length: 2048
Docker-Content-Digest: sha256:44136fa355b3678a...

fabrick_version = "1.0"
[info]
name = "product-service"
...
```

#### Step 5: Fetch WASM Module
```http
GET /v2/product-service/blobs/sha256:9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08
Host: registry.acme.io
Authorization: Bearer <token>

Response:
200 OK
Content-Type: application/vnd.fabricks.module.v1+wasm
Content-Length: 2457600
Docker-Content-Digest: sha256:9f86d081884c7d65...

<WASM binary bytes>
```

#### Step 6: Verify and Store

1. Verify SHA256 digest of downloaded content
2. Write to `~/.fabricks/registry/blobs/sha256/<digest>`
3. Update `~/.fabricks/registry/index.json`
4. Create manifest reference at `manifests/registry.acme.io/product-service/v2.1.0`

**Output:**
```
Pulling registry.acme.io/product-service:v2.1.0...
✓ Downloaded manifest
✓ Config blob (2 KB) already cached
✓ Downloaded WASM module (2.4 MB)
✓ Verified integrity
product-service:v2.1.0: Pulled
Digest: sha256:a1b2c3d4e5f6a7b8...
```

---

### Push Flow

**Command:**
```bash
fabricks push registry.acme.io/product-service:v2.1.0
```

#### Step 1: Build Manifest

1. Read Fabrickfile
2. Hash WASM module: `sha256 target/wasm32-wasi/release/product_service.wasm`
3. Hash Fabrickfile: `sha256 Fabrickfile`
4. Create manifest JSON with digests, sizes, annotations

#### Step 2: Check Existing Blobs
```http
HEAD /v2/product-service/blobs/sha256:9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08
Host: registry.acme.io
Authorization: Bearer <token>

Response:
404 Not Found (blob doesn't exist, need to upload)

OR

200 OK (blob exists, skip upload)
Content-Length: 2457600
Docker-Content-Digest: sha256:9f86d081884c7d65...
```

#### Step 3: Upload Config Blob

**Initiate Upload:**
```http
POST /v2/product-service/blobs/uploads/
Host: registry.acme.io
Authorization: Bearer <token>

Response:
202 Accepted
Location: /v2/product-service/blobs/uploads/uuid-1234-5678-90ab-cdef
Docker-Upload-UUID: uuid-1234-5678-90ab-cdef
Range: 0-0
```

**Upload Content (Monolithic):**
```http
PUT /v2/product-service/blobs/uploads/uuid-1234-5678-90ab-cdef?digest=sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a
Host: registry.acme.io
Content-Type: application/octet-stream
Content-Length: 2048
Authorization: Bearer <token>

<Fabrickfile TOML bytes>

Response:
201 Created
Location: /v2/product-service/blobs/sha256:44136fa355b3678a...
Docker-Content-Digest: sha256:44136fa355b3678a...
```

#### Step 4: Upload WASM Module

**Initiate Upload:**
```http
POST /v2/product-service/blobs/uploads/
Host: registry.acme.io
Authorization: Bearer <token>

Response:
202 Accepted
Location: /v2/product-service/blobs/uploads/uuid-abcd-ef01-2345-6789
```

**Upload in Chunks (for large files):**
```http
PATCH /v2/product-service/blobs/uploads/uuid-abcd-ef01-2345-6789
Host: registry.acme.io
Content-Type: application/octet-stream
Content-Length: 1048576
Content-Range: 0-1048575
Authorization: Bearer <token>

<First 1MB of WASM bytes>

Response:
202 Accepted
Location: /v2/product-service/blobs/uploads/uuid-abcd-ef01-2345-6789
Range: 0-1048575
```

**Complete Upload:**
```http
PUT /v2/product-service/blobs/uploads/uuid-abcd-ef01-2345-6789?digest=sha256:9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08
Host: registry.acme.io
Content-Length: 1409024
Content-Range: 1048576-2457599
Authorization: Bearer <token>

<Remaining WASM bytes>

Response:
201 Created
Location: /v2/product-service/blobs/sha256:9f86d081884c7d65...
Docker-Content-Digest: sha256:9f86d081884c7d65...
```

#### Step 5: Upload Manifest
```http
PUT /v2/product-service/manifests/v2.1.0
Host: registry.acme.io
Content-Type: application/vnd.oci.image.manifest.v1+json
Authorization: Bearer <token>

{
  "schemaVersion": 2,
  "mediaType": "application/vnd.oci.image.manifest.v1+json",
  "config": {
    "digest": "sha256:44136fa355b3678a...",
    "size": 2048
  },
  "layers": [
    {
      "digest": "sha256:9f86d081884c7d65...",
      "size": 2457600
    }
  ],
  "annotations": { ... }
}

Response:
201 Created
Location: /v2/product-service/manifests/sha256:a1b2c3d4e5f6a7b8...
Docker-Content-Digest: sha256:a1b2c3d4e5f6a7b8...
```

**Output:**
```
Pushing product-service:v2.1.0 to registry.acme.io...
✓ Config blob (2 KB) already exists
✓ Uploading WASM module (2.4 MB)
  [====================================] 100%
✓ Uploading manifest
Digest: sha256:a1b2c3d4e5f6a7b8...
```

---

## Authentication

### Credential Storage

Credentials stored in `~/.fabricks/credentials.json`:
```json
{
  "auths": {
    "registry.acme.io": {
      "auth": "dXNlcm5hbWU6cGFzc3dvcmQ="
    },
    "ghcr.io": {
      "auth": "Z2hwX2FiY2RlZmdoaWprbG1ub3BxcnN0dXZ3eHl6MTIzNDU2Nzg5MA=="
    },
    "gcr.io": {
      "auth": "X2pzb25fa2V5OnsidHlwZSI6InNlcnZpY2VfYWNjb3VudCIsLi4ufQ=="
    }
  }
}
```

**Auth field:** Base64-encoded `username:password`

### Docker Compatibility

Fabricks can read Docker credentials:
```bash
# These are compatible:
~/.docker/config.json
~/.fabricks/credentials.json

# Login works interchangeably:
docker login registry.acme.io
fabricks login registry.acme.io
```

### OAuth2 Token Flow

1. **Initial Request (Unauthenticated):**
```http
GET /v2/product-service/manifests/v2.1.0
Host: registry.acme.io

Response:
401 Unauthorized
WWW-Authenticate: Bearer realm="https://auth.acme.io/token",service="registry.acme.io",scope="repository:product-service:pull"
```

2. **Request Token:**
```http
GET /token?service=registry.acme.io&scope=repository:product-service:pull
Host: auth.acme.io
Authorization: Basic dXNlcm5hbWU6cGFzc3dvcmQ=

Response:
200 OK
Content-Type: application/json

{
  "token": "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJhdXRoLmFjbWUuaW8iLCJzdWIiOiJ1c2VyIiwiYXVkIjoicmVnaXN0cnkuYWNtZS5pbyIsImV4cCI6MTY0MjUwMDAwMCwibmJmIjoxNjQyNDk2NDAwLCJpYXQiOjE2NDI0OTY0MDAsImFjY2VzcyI6W3sidHlwZSI6InJlcG9zaXRvcnkiLCJuYW1lIjoicHJvZHVjdC1zZXJ2aWNlIiwiYWN0aW9ucyI6WyJwdWxsIl19XX0.signature",
  "access_token": "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9...",
  "expires_in": 3600,
  "issued_at": "2025-01-15T10:00:00Z"
}
```

3. **Authenticated Request:**
```http
GET /v2/product-service/manifests/v2.1.0
Host: registry.acme.io
Authorization: Bearer eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9...

Response:
200 OK
{ ... manifest ... }
```

### Scopes

Different scopes for different operations:
```
Pull:  repository:product-service:pull
Push:  repository:product-service:pull,push
Admin: repository:product-service:*
```

### Cloud Provider Authentication

#### AWS ECR
```bash
# Get token
aws ecr get-login-password --region us-east-1 | \
  fabricks login --password-stdin 123456789.dkr.ecr.us-east-1.amazonaws.com
```

#### Google GCR/Artifact Registry
```bash
# Use service account key
cat key.json | fabricks login --username _json_key --password-stdin gcr.io
```

#### GitHub Container Registry
```bash
# Use personal access token
echo $GITHUB_TOKEN | fabricks login --username USERNAME --password-stdin ghcr.io
```

---

## Registry URL Format

### URL Syntax
```
[registry/][namespace/]name[:tag|@digest]
```

### Components

- **registry** - Registry hostname (default: `registry.fabricks.io`)
- **namespace** - Organization or user namespace (default: `library`)
- **name** - Fabrick name
- **tag** - Version tag (default: `latest`)
- **digest** - SHA256 digest for immutable reference

### Examples
```bash
# Minimal (uses defaults)
fabricks pull redis:7.2
# Expands to: registry.fabricks.io/library/redis:7.2

# Explicit registry
fabricks pull registry.acme.io/product-service:v2.1.0

# With namespace
fabricks pull ghcr.io/myorg/cart-service:v1.5.0

# By digest (immutable)
fabricks pull registry.acme.io/product-service@sha256:a1b2c3d4e5f6a7b8...

# Latest tag (implicit)
fabricks pull registry.acme.io/product-service
# Same as: registry.acme.io/product-service:latest
```

### Special Protocol: `wasm://`

Convenience protocol for Fabricks registry:
```bash
# In commands
fabricks pull wasm://redis:7.2
# Resolves to: registry.fabricks.io/library/redis:7.2

# In Fabrickfiles
[from]
image = "wasm://nginx:alpine"
# Resolves to: registry.fabricks.io/library/nginx:alpine

[imports]
logger = "wasm://logger:v1.0"
# Resolves to: registry.fabricks.io/library/logger:v1.0
```

### Tag vs Digest

**Tags are mutable:**
```bash
# Tag can point to different content over time
fabricks pull registry.acme.io/api:latest
fabricks pull registry.acme.io/api:v2
```

**Digests are immutable:**
```bash
# Digest always refers to exact same content
fabricks pull registry.acme.io/api@sha256:a1b2c3d4e5f6a7b8...
```

**Best practice for production:**
```toml
# Use digests in production
[from]
image = "registry.acme.io/base@sha256:abc123..."

# Or pin to specific version tags
[from]
image = "registry.acme.io/base:v2.1.0"
```

---

## Metadata and Annotations

### Standard OCI Annotations

Following [OCI Image Spec Annotations](https://github.com/opencontainers/image-spec/blob/main/annotations.md):
```json
{
  "org.opencontainers.image.created": "2025-01-15T10:23:45Z",
  "org.opencontainers.image.authors": "backend-team@acme.com",
  "org.opencontainers.image.url": "https://acme.com/product-service",
  "org.opencontainers.image.documentation": "https://docs.acme.com/services/product",
  "org.opencontainers.image.source": "https://github.com/acme/product-service",
  "org.opencontainers.image.version": "2.1.0",
  "org.opencontainers.image.revision": "abc123def456",
  "org.opencontainers.image.vendor": "Acme Corporation",
  "org.opencontainers.image.licenses": "MIT",
  "org.opencontainers.image.ref.name": "registry.acme.io/product-service:v2.1.0",
  "org.opencontainers.image.title": "Product Service",
  "org.opencontainers.image.description": "Product catalog and inventory management service"
}
```

### Fabricks-Specific Annotations

Custom annotations under `dev.fabricks.*` namespace:
```json
{
  "dev.fabricks.version": "1.0",
  "dev.fabricks.name": "product-service",
  "dev.fabricks.platform": "wasm32-wasi",
  "dev.fabricks.runtime": "wasmtime:25.0.0",
  "dev.fabricks.exports": "[\"list_products\",\"get_product\",\"create_product\"]",
  "dev.fabricks.imports": "{\"search_indexer\":\"wasm://search/indexer:v1.0\",\"logger\":\"wasm://logger:v1.0\"}",
  "dev.fabricks.capabilities.network.listen": "[8080]",
  "dev.fabricks.capabilities.network.connect": "[\"postgres:5432\",\"redis:6379\"]",
  "dev.fabricks.capabilities.env": "[\"DATABASE_URL\",\"LOG_LEVEL\"]",
  "dev.fabricks.health_check": "{\"type\":\"http\",\"path\":\"/health\",\"interval\":\"30s\"}"
}
```

### Annotation Guidelines

**DO:**
- Use standard OCI annotations when applicable
- Use `dev.fabricks.*` prefix for Fabricks-specific data
- Keep values reasonably sized (< 1KB per annotation)
- Use JSON for structured data in annotations

**DON'T:**
- Store large data in annotations (use layers instead)
- Use arbitrary custom prefixes (use `dev.fabricks.*`)
- Duplicate data between annotations and config blob

---

## Content Verification

### SHA256 Digest Verification

Every blob is verified by SHA256 digest:

**During download:**
```bash
# Download blob
curl -o blob.tmp https://registry.acme.io/v2/.../blobs/sha256:9f86d081...

# Verify digest
echo "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08  blob.tmp" | sha256sum -c
# blob.tmp: OK

# If verification fails, reject blob
```

**During storage:**
```bash
# Compute digest
DIGEST=$(sha256sum blob.tmp | cut -d' ' -f1)

# Store by digest
mv blob.tmp ~/.fabricks/registry/blobs/sha256/$DIGEST
```

### Manifest Verification

Verify manifest digest matches header:
```bash
# Response header
Docker-Content-Digest: sha256:a1b2c3d4e5f6a7b8...

# Compute digest of response body
echo "$MANIFEST_JSON" | sha256sum
# a1b2c3d4e5f6a7b8... (must match header)
```

### Signature Verification (Cosign)

Sign fabricks with Cosign for supply chain security:

#### Sign
```bash
# Generate key pair (one-time)
cosign generate-key-pair

# Sign fabrick
cosign sign --key cosign.key registry.acme.io/product-service:v2.1.0

# Signature stored as separate OCI artifact:
# registry.acme.io/product-service:sha256-a1b2c3d4e5f6a7b8.sig
```

#### Verify
```bash
# Verify signature
cosign verify --key cosign.pub registry.acme.io/product-service:v2.1.0

# Output
Verification for registry.acme.io/product-service:v2.1.0 --
The following checks were performed on each of these signatures:
  - The cosign claims were validated
  - The signatures were verified against the specified public key

[{"critical":{"identity":{"docker-reference":"registry.acme.io/product-service"},"image":{"docker-manifest-digest":"sha256:a1b2c3d4e5f6a7b8..."}},"optional":null}]
```

#### Keyless Signing (Sigstore)
```bash
# Sign with OIDC identity (no key management)
cosign sign registry.acme.io/product-service:v2.1.0

# Verify with certificate
cosign verify \
  --certificate-identity=user@acme.com \
  --certificate-oidc-issuer=https://accounts.google.com \
  registry.acme.io/product-service:v2.1.0
```

### SBOM Attestation

Attach SBOM (Software Bill of Materials):
```bash
# Generate SBOM
syft packages ./services/product -o cyclonedx-json > sbom.json

# Attach to fabrick
cosign attach sbom --sbom sbom.json registry.acme.io/product-service:v2.1.0

# Verify SBOM exists
cosign verify-attestation registry.acme.io/product-service:v2.1.0
```

---

## Registry Configuration

### Default Configuration

**`~/.fabricks/config.toml`**
```toml
[registry]
# Default registry for unqualified names
default = "registry.fabricks.io"

# Default namespace
default_namespace = "library"

# Insecure registries (HTTP instead of HTTPS)
insecure = ["localhost:5000", "registry.local"]

# Skip TLS verification (not recommended)
insecure_skip_verify = []

# Registry mirrors for offline/air-gapped
[[registry.mirrors]]
url = "https://mirror.internal.corp"
insecure = false

# Per-registry configuration
[[registry.config]]
url = "registry.acme.io"
insecure = false
ca_cert = "/etc/ssl/certs/acme-ca.crt"
client_cert = "/etc/ssl/certs/client.crt"
client_key = "/etc/ssl/private/client.key"

[[registry.config]]
url = "localhost:5000"
insecure = true
```

### Environment Variables

Override config with environment variables:
```bash
# Default registry
export FABRICKS_REGISTRY=registry.acme.io

# Credentials
export FABRICKS_REGISTRY_USER=myuser
export FABRICKS_REGISTRY_PASSWORD=mypassword

# Insecure registry
export FABRICKS_INSECURE_REGISTRY=localhost:5000

# CA certificate
export FABRICKS_CA_CERT=/path/to/ca.crt
```

### Registry Aliases

Define short aliases:
```toml
[registry.aliases]
acme = "registry.acme.io/myorg"
internal = "registry.internal.corp/fabricks"
dev = "localhost:5000"
```

**Usage:**
```bash
# Instead of:
fabricks pull registry.acme.io/myorg/product-service:v2.1.0

# Use alias:
fabricks pull acme/product-service:v2.1.0
```

---

## Supported Registries

Fabricks works with any OCI-compliant registry:

### Public Registries

#### Docker Hub
```bash
fabricks pull docker.io/library/redis:7.2
fabricks push docker.io/myuser/my-service:v1.0.0
```

#### GitHub Container Registry (GHCR)
```bash
fabricks pull ghcr.io/myorg/cart-service:v1.5.0
fabricks push ghcr.io/myorg/my-service:v1.0.0
```

#### Google Container Registry (GCR)
```bash
fabricks pull gcr.io/myproject/my-service:v1.0.0
fabricks push gcr.io/myproject/my-service:v1.0.0
```

#### Google Artifact Registry
```bash
fabricks pull us-docker.pkg.dev/myproject/myrepo/my-service:v1.0.0
fabricks push us-docker.pkg.dev/myproject/myrepo/my-service:v1.0.0
```

#### Amazon ECR Public
```bash
fabricks pull public.ecr.aws/myorg/my-service:v1.0.0
fabricks push public.ecr.aws/myorg/my-service:v1.0.0
```

### Private/Enterprise Registries

#### Amazon ECR
```bash
fabricks pull 123456789.dkr.ecr.us-east-1.amazonaws.com/my-service:v1.0.0
fabricks push 123456789.dkr.ecr.us-east-1.amazonaws.com/my-service:v1.0.0
```

#### Azure Container Registry (ACR)
```bash
fabricks pull myregistry.azurecr.io/my-service:v1.0.0
fabricks push myregistry.azurecr.io/my-service:v1.0.0
```

#### JFrog Artifactory
```bash
fabricks pull artifactory.acme.io/docker-local/my-service:v1.0.0
fabricks push artifactory.acme.io/docker-local/my-service:v1.0.0
```

#### Harbor
```bash
fabricks pull harbor.acme.io/library/my-service:v1.0.0
fabricks push harbor.acme.io/library/my-service:v1.0.0
```

#### Sonatype Nexus
```bash
fabricks pull nexus.acme.io:5000/my-service:v1.0.0
fabricks push nexus.acme.io:5000/my-service:v1.0.0
```

### Self-Hosted

#### Distribution (registry:2)
```bash
# Run registry
docker run -d -p 5000:5000 --name registry registry:2

# Use with Fabricks
fabricks push localhost:5000/my-service:v1.0.0
fabricks pull localhost:5000/my-service:v1.0.0
```

---

## Examples

### Example 1: Pull from Docker Hub
```bash
# Pull official Redis fabrick
fabricks pull wasm://redis:7.2

# Verify contents
fabricks inspect wasm://redis:7.2
```

**Output:**
```
Pulling registry.fabricks.io/library/redis:7.2...
✓ Downloaded manifest
✓ Downloaded config (1.2 KB)
✓ Downloaded WASM module (4.1 MB)
✓ Verified integrity
redis:7.2: Pulled
Digest: sha256:abc123def456...
```

### Example 2: Build and Push to Private Registry
```bash
# Build fabrick
fabricks build -t my-service:v1.0.0 ./services/api

# Tag for private registry
fabricks build -t registry.acme.io/myorg/my-service:v1.0.0 ./services/api

# Login to registry
fabricks login registry.acme.io

# Push
fabricks push registry.acme.io/myorg/my-service:v1.0.0
```

**Output:**
```
Building my-service:v1.0.0...
✓ Built my-service.wasm (2.3 MB)

Pushing my-service:v1.0.0 to registry.acme.io...
✓ Config blob (2 KB) already exists
✓ Uploading WASM module (2.3 MB)
  [====================================] 100%
✓ Uploading manifest
Digest: sha256:9f86d081884c7d65...
```

### Example 3: Multi-architecture Build
```bash
# Build for wasm32-wasi
fabricks build --platform wasm32-wasi -t my-service:v1.0.0-wasi

# Build for wasm32-unknown-unknown
fabricks build --platform wasm32-unknown-unknown -t my-service:v1.0.0-unknown

# Create multi-platform manifest
fabricks manifest create my-service:v1.0.0 \
  my-service:v1.0.0-wasi \
  my-service:v1.0.0-unknown

# Push all
fabricks push my-service:v1.0.0
```

### Example 4: Air-gapped Deployment
```bash
# On internet-connected machine:
# Pull all required fabricks
fabricks pull registry.fabricks.io/redis:7.2
fabricks pull registry.fabricks.io/postgres:16
fabricks pull registry.acme.io/my-service:v1.0.0

# Export to tar
fabricks save -o fabricks.tar \
  registry.fabricks.io/redis:7.2 \
  registry.fabricks.io/postgres:16 \
  registry.acme.io/my-service:v1.0.0

# On air-gapped machine:
# Import from tar
fabricks load -i fabricks.tar

# Push to local registry
fabricks push registry.local/redis:7.2
fabricks push registry.local/postgres:16
fabricks push registry.local/my-service:v1.0.0
```

### Example 5: Signing and Verification
```bash
# Generate signing key
cosign generate-key-pair

# Build and push
fabricks build -t registry.acme.io/my-service:v1.0.0
fabricks push registry.acme.io/my-service:v1.0.0

# Sign
cosign sign --key cosign.key registry.acme.io/my-service:v1.0.0

# Later: verify before deployment
cosign verify --key cosign.pub registry.acme.io/my-service:v1.0.0

# Pull only if verified
fabricks pull registry.acme.io/my-service:v1.0.0
```

---

## Summary

Fabricks registry specification provides:

✅ **Standard Protocol** - OCI Distribution Specification  
✅ **Content-addressable** - SHA256 digests for integrity and deduplication  
✅ **Compatible** - Works with Docker Hub, GHCR, ECR, GCR, Harbor, Artifactory  
✅ **Secure** - Bearer token auth + Cosign signatures  
✅ **Efficient** - Chunked uploads, resumable downloads, layer sharing  

**Key Design Decisions:**
- Use OCI Artifacts (not container images)
- Custom media types for WASM and Fabrickfiles
- Standard OCI annotations + Fabricks-specific annotations
- Local storage follows OCI Image Layout spec
