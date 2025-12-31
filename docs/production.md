# Production Best Practices

Guidelines for running Fabricks in production environments.

---

## Overview

This guide covers best practices for deploying and operating Fabricks applications in production, including:

- High availability configuration
- Security hardening
- Monitoring and observability
- Performance tuning
- Operational procedures

---

## High Availability

### Service Replicas

Always run multiple replicas of critical services:

```toml
[service.api]
build = "./services/api"

[service.api.replicas]
min = 3        # Minimum replicas
max = 20       # Maximum for autoscaling
cpu_threshold = 70  # Scale up at 70% CPU
```

### Database Considerations

For stateful services:

```toml
[service.postgres]
image = "wasm://pglite:latest"

[service.postgres.replicas]
min = 1  # Primary
max = 1  # No autoscaling for databases

[service.postgres.backup]
enabled = true
schedule = "0 */6 * * *"  # Every 6 hours
retention = "30d"
destination = "s3://backups/postgres"
```

For true HA, consider external managed databases (RDS, Cloud SQL) and connect via capabilities.

### Health Checks

Configure thorough health checks:

```toml
[service.api.health_check.http]
path = "/health"
port = 8080
interval = "10s"
timeout = "5s"
retries = 3
expected_status = 200
```

Health endpoints should verify:
- Application is running
- Database connectivity
- Cache connectivity
- Critical dependencies

### Restart Policies

Configure restart behavior:

```toml
[service.api.restart]
policy = "on-failure"  # always | on-failure | never
max_attempts = 5
backoff = "30s"        # Wait between restarts
```

---

## Security Hardening

### Minimal Capabilities

Grant only required capabilities:

```toml
[capabilities]
# Only needed environment variables
env = ["DATABASE_URL", "LOG_LEVEL"]

[capabilities.network]
# Only required ports
listen = [8080]
# Only required hosts
connect = ["postgres:5432", "redis:6379"]

[capabilities.filesystem]
# Only required paths
read = ["./config"]
# Avoid write access unless necessary
```

### Network Isolation

Use strict network segmentation:

```toml
[network.public]
description = "Internet-facing"
ingress = "0.0.0.0/0"
egress = ["application"]

[network.application]
internal = true
ingress = ["public"]
egress = ["data"]

[network.data]
internal = true
ingress = ["application"]
# No egress - databases shouldn't initiate connections
```

### Secrets Management

Never store secrets in configuration:

```toml
# Good - reference external secrets
[secret.db_password]
provider = "vault"
path = "secret/data/postgres"
key = "password"

[service.api]
environment = {
    DB_PASSWORD = "${secret.db_password}"
}
```

For Kubernetes:

```toml
[secret.api_key]
provider = "kubernetes"
name = "api-credentials"
key = "api_key"
```

### Security Settings

Enable security hardening:

```toml
[service.api.security]
read_only_root = true    # Immutable filesystem
egress_locked = true     # Only allowed hosts
tls_required = true      # Require TLS
secrets_encrypted = true # Encrypt at rest
```

### PCI/HIPAA Compliance

For regulated workloads:

```toml
[network.payment]
isolated = true          # No cross-network traffic
audit_all = true         # Log all traffic
encryption = "required"  # Mandatory TLS

[service.payment.audit]
enabled = true
log_level = "verbose"
pii_redact = true        # Redact sensitive data

[policy.pci_compliance]
[[policy.pci_compliance.deny]]
from = ["payment"]
to = ["monitoring"]
reason = "PCI data must not be logged"

[[policy.pci_compliance.require]]
networks = ["payment"]
tls = true
audit = true
```

---

## Monitoring and Observability

### Metrics

Export metrics for monitoring:

```toml
[service.api]
# Prometheus metrics endpoint
[service.api.metrics]
enabled = true
path = "/metrics"
port = 9090
```

Key metrics to monitor:
- Request rate and latency
- Error rates
- CPU and memory usage
- Connection pool utilization
- Queue depths

### Logging

Configure structured logging:

```toml
[service.api]
environment = {
    LOG_LEVEL = "info",
    LOG_FORMAT = "json"
}
```

Aggregate logs with your preferred solution (ELK, Loki, CloudWatch).

### Event Streaming

Monitor system events:

```bash
# Stream all events
fabricks events --format json | your-log-shipper

# Critical events only
fabricks events --types service.failed,health.changed
```

### Health Dashboard

Monitor service health:

```bash
# Real-time status
fabricks mortar ps

# Detailed stats
fabricks daemon stats
```

### Alerting

Set up alerts for:
- Service failures (`service.failed`)
- Health check failures (`health.changed`)
- High resource usage
- Scaling events
- Policy violations

---

## Performance Tuning

### Resource Allocation

Size resources appropriately:

```toml
[service.api.resources]
memory = "512Mi"  # Based on actual usage + headroom
cpu = 1.0         # Based on load testing

[service.worker.resources]
memory = "256Mi"
cpu = 0.5
```

Monitor actual usage and adjust:

```bash
fabricks service stats api
```

### Connection Pooling

Configure connection pools:

```toml
[service.api]
environment = {
    DATABASE_URL = "postgres://db:5432/app?pool_size=20&pool_timeout=30"
}
```

### Caching

Use caching effectively:

```toml
[service.api]
networks = ["application", "cache"]

[service.api.imports]
cache = { service = "redis", interface = "cache" }

[capabilities.network]
connect = ["redis:6379"]
```

### Autoscaling

Configure autoscaling based on load testing:

```toml
[service.api.replicas]
min = 3           # Handle baseline load
max = 50          # Handle peak load
cpu_threshold = 70
# Scale up before hitting limits

# Cooldown to prevent thrashing
scale_up_cooldown = "60s"
scale_down_cooldown = "300s"
```

---

## Operational Procedures

### Deployment

#### Rolling Updates

```bash
# Build new version
fabricks mortar build

# Deploy with zero downtime
fabricks mortar up --build

# Or gradual rollout
fabricks mortar scale api=1  # Deploy to 1 instance
# Verify health
fabricks mortar scale api=10 # Scale to full
```

#### Rollback

```bash
# Keep previous images tagged
fabricks build -t api:v1.2.0 -t api:v1.2.0-previous

# Rollback if needed
fabricks mortar down
# Edit mortar file to previous version
fabricks mortar up
```

### Scaling

```bash
# Manual scaling
fabricks mortar scale api=10

# View current scale
fabricks mortar ps
```

### Maintenance

#### Draining Services

```bash
# Graceful shutdown
fabricks mortar down --timeout 60

# Service by service
fabricks mortar stop api --timeout 60
```

#### Backup Verification

```bash
# List backups
fabricks volume backup list postgres_data

# Test restore
fabricks volume backup restore postgres_data --target test_restore
```

### Troubleshooting

```bash
# Check service status
fabricks service inspect api

# View logs
fabricks service logs api --tail 100

# Check events
fabricks events --service api --since 1h

# Resource usage
fabricks service stats api

# Network connectivity
fabricks network inspect application
```

---

## Disaster Recovery

### Backup Strategy

```toml
[volume.postgres_data.backup]
enabled = true
schedule = "0 */4 * * *"  # Every 4 hours
retention = "30d"
destination = "s3://backups/postgres"

# Also backup to secondary region
[[volume.postgres_data.backup.replicate]]
destination = "s3://backups-dr/postgres"
region = "us-west-2"
```

### Recovery Procedures

1. **Service failure** - Automatic restart via restart policy
2. **Node failure** - Kubernetes reschedules pods
3. **Zone failure** - Deploy across multiple zones
4. **Region failure** - Restore from cross-region backups

### RTO/RPO Targets

Document and test:
- **RTO** (Recovery Time Objective) - How fast can you recover?
- **RPO** (Recovery Point Objective) - How much data can you lose?

---

## Checklist

### Pre-Production

- [ ] All services have health checks
- [ ] Resource limits configured
- [ ] Replicas set for HA
- [ ] Network segmentation in place
- [ ] Secrets externalized
- [ ] Backups configured and tested
- [ ] Monitoring and alerting set up
- [ ] Load testing completed
- [ ] Security review completed
- [ ] Runbooks documented

### Go-Live

- [ ] DNS configured
- [ ] TLS certificates provisioned
- [ ] Monitoring dashboards ready
- [ ] On-call rotation scheduled
- [ ] Rollback procedure tested
- [ ] Stakeholders notified

### Post-Launch

- [ ] Monitor error rates
- [ ] Watch resource utilization
- [ ] Review autoscaling behavior
- [ ] Verify backup execution
- [ ] Update documentation

---

## Configuration Template

A production-ready mortar file:

```toml
mortar_version = "1.0"

[project]
name = "my-app"
version = "1.0.0"

# Secrets from Vault
[secret.db_password]
provider = "vault"
path = "secret/data/postgres"

[secret.api_key]
provider = "vault"
path = "secret/data/api"

# Network segmentation
[network.public]
ingress = "0.0.0.0/0"
egress = ["application"]

[network.application]
internal = true
ingress = ["public"]
egress = ["data", "cache"]

[network.data]
internal = true
ingress = ["application"]

[network.cache]
internal = true
ingress = ["application"]

# API Service
[service.api]
build = "./services/api"
networks = ["public", "application"]
ports = ["8080:8080"]
depends_on = ["postgres", "redis"]

environment = {
    DATABASE_URL = "postgres://postgres:5432/app",
    REDIS_URL = "redis://redis:6379",
    DB_PASSWORD = "${secret.db_password}",
    LOG_LEVEL = "info",
    LOG_FORMAT = "json"
}

[service.api.replicas]
min = 3
max = 20
cpu_threshold = 70

[service.api.resources]
memory = "512Mi"
cpu = 1.0

[service.api.health_check.http]
path = "/health"
interval = "10s"
timeout = "5s"
retries = 3

[service.api.restart]
policy = "on-failure"
max_attempts = 5
backoff = "30s"

[service.api.security]
read_only_root = true
tls_required = true

# Database
[service.postgres]
image = "wasm://pglite:latest"
networks = ["data"]

environment = {
    POSTGRES_PASSWORD = "${secret.db_password}"
}

[service.postgres.volumes]
postgres_data = "/var/lib/postgresql/data"

[service.postgres.replicas]
min = 1
max = 1

[service.postgres.resources]
memory = "2Gi"
cpu = 2.0

[service.postgres.health_check.tcp]
port = 5432
interval = "10s"

[service.postgres.backup]
enabled = true
schedule = "0 */4 * * *"
retention = "30d"
destination = "s3://backups/postgres"

# Cache
[service.redis]
image = "wasm://redis:7.2"
networks = ["cache"]

[service.redis.replicas]
min = 2
max = 2

[service.redis.resources]
memory = "256Mi"
cpu = 0.5

[service.redis.health_check.tcp]
port = 6379
interval = "10s"

# Volumes
[volume.postgres_data]
size = "100Gi"
encrypted = true

[volume.postgres_data.backup]
enabled = true
schedule = "0 */4 * * *"
retention = "30d"

# Validation
[validate]
require_health_checks = true
deny_wildcard_connect = true
require_explicit_capabilities = true
scan_dependencies = true

# Production profile
[profile.production]
target = "kubernetes"
namespace = "my-app"

[profile.production.settings]
high_availability = true
enable_monitoring = true
```

---

## Related Documentation

- [Kubernetes](kubernetes.md) - Kubernetes deployment
- [Networking](networking.md) - Network segmentation
- [Capabilities](capabilities.md) - Security model
- [CLI Reference](cli-reference.md) - Operations commands
