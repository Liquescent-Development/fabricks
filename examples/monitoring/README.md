# Monitoring Stack Example

A complete observability stack with Prometheus and Grafana for monitoring Fabricks applications.

## Structure

```
monitoring/
├── fabricks-mortar.toml      # Multi-service composition
├── config/
│   ├── prometheus.yml        # Prometheus configuration
│   └── grafana/
│       └── dashboards/       # Grafana dashboard definitions
└── README.md
```

## Quick Start

```bash
# Start the daemon
fabricks daemon start

# Build and run the monitoring stack
fabricks mortar up --build

# Access the dashboards
# Prometheus: http://localhost:9090
# Grafana:    http://localhost:3000 (admin/admin)

# View status
fabricks mortar ps

# Stop everything
fabricks mortar down
```

## What This Demonstrates

- Monitoring network pattern (ingress-only)
- Prometheus metrics collection
- Grafana visualization
- Persistent storage for metrics
- Service discovery via configuration

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                   [application network]                      │
│                                                              │
│   ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐   │
│   │  api-1   │  │  api-2   │  │ worker-1 │  │ worker-2 │   │
│   │ :8080    │  │ :8080    │  │          │  │          │   │
│   │ /metrics │  │ /metrics │  │ /metrics │  │ /metrics │   │
│   └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘   │
└────────┼─────────────┼─────────────┼─────────────┼──────────┘
         │             │             │             │
         └─────────────┴──────┬──────┴─────────────┘
                              │ scrape
┌─────────────────────────────▼───────────────────────────────┐
│                   [monitoring network]                       │
│                                                              │
│   ┌──────────────────────────────────────────────────────┐  │
│   │                    Prometheus                         │  │
│   │              (metrics collection)                     │  │
│   │                                                       │  │
│   │  Volume: prometheus_data (50Gi)                       │  │
│   └────────────────────────┬─────────────────────────────┘  │
│                            │                                 │
│   ┌────────────────────────▼─────────────────────────────┐  │
│   │                     Grafana                           │  │
│   │               (visualization)                         │  │
│   │                                                       │  │
│   │  Volume: grafana_data (5Gi)                           │  │
│   └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
         │
         ▼ :3000
    ┌─────────┐
    │ Browser │
    └─────────┘
```

## Key Features

### Ingress-Only Network

The monitoring network can receive metrics from all other networks but cannot initiate connections to them:

```toml
[network.monitoring]
description = "Observability services"
ingress = ["*"]           # All networks can send metrics
ingress_only = true       # Cannot initiate connections
```

### Prometheus Configuration

```toml
[service.prometheus]
image = "wasm://prometheus:v2.50"
networks = ["monitoring"]
ports = ["9090:9090"]

[service.prometheus.files]
"./config/prometheus.yml" = "/etc/prometheus/prometheus.yml"

[service.prometheus.volumes]
prometheus_data = "/prometheus"
```

### Grafana Configuration

```toml
[service.grafana]
image = "wasm://grafana:10"
networks = ["monitoring"]
ports = ["3000:3000"]
depends_on = ["prometheus"]

environment = {
    GF_SECURITY_ADMIN_PASSWORD = "${secret.grafana_admin_password}"
}

[service.grafana.volumes]
grafana_data = "/var/lib/grafana"
```

## Metrics Exposure

To expose metrics from your services, add a `/metrics` endpoint:

```toml
# In your service's Fabrickfile
[capabilities.network]
listen = [8080, 9090]  # 9090 for metrics

# Prometheus will scrape this endpoint
```

## Prometheus Configuration

```yaml
# config/prometheus.yml
global:
  scrape_interval: 15s
  evaluation_interval: 15s

scrape_configs:
  - job_name: 'fabricks-services'
    static_configs:
      - targets:
        - 'api:9090'
        - 'worker:9090'
    metrics_path: /metrics
```

## Grafana Dashboards

Pre-configured dashboards are loaded from `config/grafana/dashboards/`:

- **Service Overview** - Request rate, latency, error rate
- **Resource Usage** - CPU, memory, network
- **Health Status** - Service health, replica counts

## Alerting

Configure alerts in Prometheus:

```yaml
# config/prometheus.yml
rule_files:
  - /etc/prometheus/alerts/*.yml

alerting:
  alertmanagers:
    - static_configs:
        - targets: ['alertmanager:9093']
```

## Integrating with Your Application

Add the monitoring network to your existing mortar file:

```toml
[network.monitoring]
ingress = ["*"]
ingress_only = true

[service.your-api]
networks = ["application", "monitoring"]

[service.prometheus]
image = "wasm://prometheus:v2.50"
networks = ["monitoring"]
```

## Next Steps

- Review [Production docs](../../docs/production.md) for production monitoring
- Add alerting with Alertmanager
- Configure log aggregation with Loki
