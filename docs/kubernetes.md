# Kubernetes Integration

Deploy Fabricks applications to Kubernetes clusters.

---

## Overview

Fabricks can generate Kubernetes manifests from your `fabricks-mortar.toml` or deploy directly to a cluster. This enables:

- **Familiar operations** - Use kubectl, Helm, ArgoCD
- **Cluster features** - Ingress, service mesh, secrets management
- **SpinKube support** - Native WASM execution on Kubernetes
- **Hybrid deployments** - Mix Fabricks with traditional containers

---

## Quick Start

### Generate Manifests

```bash
# Generate K8s manifests
fabricks k8s generate

# Output to specific directory
fabricks k8s generate -o ./manifests

# With namespace
fabricks k8s generate --namespace production
```

### Apply to Cluster

```bash
# Apply directly
fabricks k8s apply

# Or use kubectl
kubectl apply -f ./k8s/
```

---

## Generated Resources

Fabricks generates standard Kubernetes resources:

### From Services

| Fabricks | Kubernetes |
|----------|------------|
| `[service.*]` | Deployment + Service |
| `[service.*.replicas]` | HPA (HorizontalPodAutoscaler) |
| `[service.*.resources]` | Resource requests/limits |
| `[service.*.health_check]` | Liveness/readiness probes |

### From Networks

| Fabricks | Kubernetes |
|----------|------------|
| `[network.*]` | NetworkPolicy |
| `internal = true` | No external ingress |
| `ingress = [...]` | Ingress rules |
| `egress = [...]` | Egress rules |

### From Volumes

| Fabricks | Kubernetes |
|----------|------------|
| `[volume.*]` | PersistentVolumeClaim |
| `size = "10Gi"` | storage request |
| `encrypted = true` | StorageClass with encryption |

---

## Example Generation

Given this mortar file:

```toml
mortar_version = "1.0"

[project]
name = "my-app"

[service.api]
build = "./services/api"
networks = ["backend"]
ports = ["8080:8080"]

[service.api.replicas]
min = 2
max = 10
cpu_threshold = 70

[service.api.resources]
memory = "512Mi"
cpu = 1.0

[service.api.health_check.http]
path = "/health"
interval = "30s"

[service.postgres]
image = "wasm://pglite:latest"
networks = ["backend"]

[service.postgres.volumes]
data = "/var/lib/postgresql/data"

[network.backend]
internal = true

[volume.data]
size = "10Gi"
```

### Generated Deployment

```yaml
# k8s/api-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: api
  namespace: my-app
  labels:
    app: api
spec:
  replicas: 2
  selector:
    matchLabels:
      app: api
  template:
    metadata:
      labels:
        app: api
    spec:
      containers:
      - name: api
        image: my-app/api:latest
        ports:
        - containerPort: 8080
        resources:
          requests:
            memory: "512Mi"
            cpu: "1000m"
          limits:
            memory: "512Mi"
            cpu: "1000m"
        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          periodSeconds: 30
        readinessProbe:
          httpGet:
            path: /health
            port: 8080
          periodSeconds: 10
```

### Generated Service

```yaml
# k8s/api-service.yaml
apiVersion: v1
kind: Service
metadata:
  name: api
  namespace: my-app
spec:
  selector:
    app: api
  ports:
  - port: 8080
    targetPort: 8080
```

### Generated HPA

```yaml
# k8s/api-hpa.yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: api
  namespace: my-app
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: api
  minReplicas: 2
  maxReplicas: 10
  metrics:
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 70
```

### Generated NetworkPolicy

```yaml
# k8s/backend-networkpolicy.yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: backend
  namespace: my-app
spec:
  podSelector:
    matchLabels:
      network: backend
  policyTypes:
  - Ingress
  - Egress
  ingress:
  - from:
    - podSelector:
        matchLabels:
          network: backend
  egress:
  - to:
    - podSelector:
        matchLabels:
          network: backend
```

### Generated PVC

```yaml
# k8s/data-pvc.yaml
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: data
  namespace: my-app
spec:
  accessModes:
  - ReadWriteOnce
  resources:
    requests:
      storage: 10Gi
```

---

## SpinKube Integration

For native WASM execution on Kubernetes, use SpinKube:

```bash
fabricks k8s generate --use-spinkube
```

This generates SpinKube-specific resources:

```yaml
# k8s/api-spinapp.yaml
apiVersion: core.spinoperator.dev/v1alpha1
kind: SpinApp
metadata:
  name: api
  namespace: my-app
spec:
  image: my-app/api:latest
  replicas: 2
  executor: containerd-shim-spin
  resources:
    limits:
      memory: 512Mi
      cpu: 1000m
```

### Prerequisites for SpinKube

1. Install SpinKube operator:
```bash
kubectl apply -f https://github.com/spinkube/spin-operator/releases/latest/download/spin-operator.yaml
```

2. Install containerd-shim-spin on nodes

3. Configure RuntimeClass:
```yaml
apiVersion: node.k8s.io/v1
kind: RuntimeClass
metadata:
  name: wasmtime-spin
handler: spin
```

---

## Deployment Profiles

Use profiles for different environments:

```toml
# fabricks-mortar.toml

[profile.staging]
target = "kubernetes"
cluster = "staging-cluster"
namespace = "my-app-staging"

[profile.staging.overrides]
all_services = {
    replicas = { min = 1, max = 3 },
    resources = { memory = "256Mi", cpu = 0.5 }
}

[profile.production]
target = "kubernetes"
cluster = "prod-cluster"
namespace = "my-app"

[profile.production.settings]
high_availability = true
enable_monitoring = true
```

Deploy with profile:

```bash
# Generate for staging
fabricks k8s generate --profile staging -o ./k8s/staging

# Generate for production
fabricks k8s generate --profile production -o ./k8s/production

# Apply directly
fabricks k8s apply --profile production
```

---

## Secrets Management

### From Kubernetes Secrets

```toml
[secret.db_password]
provider = "kubernetes"
name = "postgres-credentials"
key = "password"
```

Generated reference:

```yaml
env:
- name: DB_PASSWORD
  valueFrom:
    secretKeyRef:
      name: postgres-credentials
      key: password
```

### From External Secrets Operator

```toml
[secret.api_key]
provider = "external-secrets"
store = "vault-backend"
key = "api/credentials"
property = "api_key"
```

---

## Ingress Configuration

### Basic Ingress

```toml
[service.api]
build = "./services/api"
networks = ["public"]
ports = ["8080:8080"]

[service.api.ingress]
host = "api.example.com"
path = "/"
tls = true
```

Generated:

```yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: api
  namespace: my-app
  annotations:
    kubernetes.io/ingress.class: nginx
    cert-manager.io/cluster-issuer: letsencrypt-prod
spec:
  tls:
  - hosts:
    - api.example.com
    secretName: api-tls
  rules:
  - host: api.example.com
    http:
      paths:
      - path: /
        pathType: Prefix
        backend:
          service:
            name: api
            port:
              number: 8080
```

### Multiple Paths

```toml
[service.api.ingress]
host = "example.com"

[[service.api.ingress.paths]]
path = "/api"
service = "api"

[[service.api.ingress.paths]]
path = "/admin"
service = "admin"
```

---

## Service Mesh Integration

### Istio

```bash
fabricks k8s generate --service-mesh istio
```

Adds Istio sidecar injection:

```yaml
metadata:
  annotations:
    sidecar.istio.io/inject: "true"
```

### Linkerd

```bash
fabricks k8s generate --service-mesh linkerd
```

Adds Linkerd annotations:

```yaml
metadata:
  annotations:
    linkerd.io/inject: enabled
```

---

## CI/CD Integration

### GitHub Actions

```yaml
# .github/workflows/deploy.yml
name: Deploy to Kubernetes

on:
  push:
    branches: [main]

jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
    - uses: actions/checkout@v4

    - name: Install Fabricks
      run: curl -fsSL https://get.fabricks.dev | sh

    - name: Build services
      run: fabricks mortar build --parallel

    - name: Push to registry
      run: |
        fabricks login ${{ secrets.REGISTRY_URL }} -u ${{ secrets.REGISTRY_USER }}
        fabricks mortar push

    - name: Generate manifests
      run: fabricks k8s generate --namespace production -o ./k8s

    - name: Deploy
      run: kubectl apply -f ./k8s/
      env:
        KUBECONFIG: ${{ secrets.KUBECONFIG }}
```

### ArgoCD

```yaml
# argocd/application.yaml
apiVersion: argoproj.io/v1alpha1
kind: Application
metadata:
  name: my-app
spec:
  project: default
  source:
    repoURL: https://github.com/org/my-app
    targetRevision: HEAD
    path: k8s
  destination:
    server: https://kubernetes.default.svc
    namespace: my-app
  syncPolicy:
    automated:
      prune: true
      selfHeal: true
```

---

## Commands Reference

### Generate

```bash
fabricks k8s generate [OPTIONS]

Options:
    -f, --file <FILE>       Mortar file [default: fabricks-mortar.toml]
    -o, --output <DIR>      Output directory [default: ./k8s]
        --namespace <NS>    Kubernetes namespace
        --profile <PROFILE> Deployment profile
        --use-spinkube      Generate SpinKube resources
        --service-mesh <SM> Add service mesh config [istio, linkerd]
```

### Apply

```bash
fabricks k8s apply [OPTIONS]

Options:
    -f, --file <FILE>       Mortar file [default: fabricks-mortar.toml]
        --namespace <NS>    Kubernetes namespace
        --context <CTX>     Kubernetes context
        --profile <PROFILE> Deployment profile
        --dry-run           Show what would be applied
```

---

## Best Practices

1. **Use namespaces** - Isolate applications in dedicated namespaces
2. **Set resource limits** - Always define memory and CPU limits
3. **Enable NetworkPolicies** - Map Fabricks network segmentation to K8s
4. **Use secrets properly** - Never hardcode secrets in manifests
5. **Configure health checks** - Essential for rolling updates
6. **Set PodDisruptionBudgets** - Ensure availability during maintenance
7. **Use GitOps** - Store manifests in version control

---

## Related Documentation

- [Production](production.md) - Production deployment practices
- [Networking](networking.md) - Network segmentation
- [CLI Reference](cli-reference.md) - Complete command reference
