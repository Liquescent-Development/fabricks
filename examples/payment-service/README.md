# PCI-Compliant Payment Service Example

An isolated payment processing service demonstrating security best practices for PCI-DSS compliance.

## Structure

```
payment-service/
├── fabricks-mortar.toml      # Multi-service composition
├── services/
│   ├── payment-gateway/      # Internal payment gateway
│   └── payment-processor/    # PCI-isolated processor
└── README.md
```

## Quick Start

```bash
# Start the daemon
fabricks daemon start

# Build and run all services
fabricks mortar up --build

# View service status
fabricks mortar ps

# View audit logs
fabricks mortar logs payment-processor

# Stop everything
fabricks mortar down
```

## What This Demonstrates

- **Isolated network** - Payment zone completely isolated from other services
- **Audit logging** - All traffic logged for compliance
- **Encrypted communication** - TLS required for all connections
- **Minimal capabilities** - Only Stripe API access allowed
- **Read-only filesystem** - Immutable runtime environment
- **PII redaction** - Sensitive data redacted in logs

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     [application network]                    │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │                  Order Service                        │   │
│  │           (initiates payment requests)                │   │
│  └─────────────────────────┬────────────────────────────┘   │
└────────────────────────────┼────────────────────────────────┘
                             │
┌────────────────────────────▼────────────────────────────────┐
│                  [payment-bridge network]                    │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │               Payment Gateway                         │   │
│  │      (validates, routes payment requests)             │   │
│  └─────────────────────────┬────────────────────────────┘   │
└────────────────────────────┼────────────────────────────────┘
                             │
┌────────────────────────────▼────────────────────────────────┐
│                    [payment network]                         │
│                     🔒 ISOLATED 🔒                           │
│                     📝 AUDITED 📝                            │
│                     🔐 ENCRYPTED 🔐                          │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │              Payment Processor                        │   │
│  │        (PCI-DSS compliant processing)                 │   │
│  │                                                       │   │
│  │  • Only connects to api.stripe.com:443                │   │
│  │  • Read-only filesystem                               │   │
│  │  • Secrets encrypted at rest                          │   │
│  │  • All traffic audited                                │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                             │
                             ▼
                    ┌─────────────────┐
                    │  api.stripe.com │
                    │    (external)   │
                    └─────────────────┘
```

## Key Security Features

### Isolated Network

```toml
[network.payment]
description = "PCI-DSS compliant payment zone"
isolated = true              # Cannot talk to other internal networks
audit_all = true             # Log all traffic
encryption = "required"      # TLS mandatory
egress = ["external:payment-gateways"]
```

### External Hosts Whitelist

```toml
[external_hosts.payment-gateways]
description = "Approved payment processors"
hosts = [
    "api.stripe.com:443",
    "api.braintreegateway.com:443"
]
tls_required = true
```

### Security Policies

```toml
[policy.pci_compliance]
description = "Enforce PCI-DSS requirements"

[[policy.pci_compliance.deny]]
from = ["payment"]
to = ["monitoring"]
reason = "PCI data must not be logged to general monitoring"

[[policy.pci_compliance.deny]]
from = ["payment"]
to = ["external:*"]
except = ["external:payment-gateways"]
reason = "Payment zone can only reach approved gateways"

[[policy.pci_compliance.require]]
networks = ["payment"]
tls = true
audit = true
```

### Service Security Hardening

```toml
[service.payment-processor.security]
egress_locked = true         # Only whitelisted hosts
secrets_encrypted = true     # Encrypt secrets at rest
tls_required = true          # All connections use TLS
read_only_root = true        # Immutable filesystem
```

### Audit Logging

```toml
[service.payment-processor.audit]
enabled = true
log_level = "verbose"        # Log everything
pii_redact = true            # Redact credit card numbers, etc.
```

## Secrets Management

```toml
[secret.stripe_api_key]
provider = "vault"
path = "secret/data/stripe"
key = "api_key"

[secret.stripe_webhook_secret]
provider = "vault"
path = "secret/data/stripe"
key = "webhook_secret"
```

Never store secrets in configuration files!

## Compliance Checklist

- [x] Network isolation (Requirement 1)
- [x] Encrypted transmission (Requirement 4)
- [x] Access controls (Requirement 7)
- [x] Audit logging (Requirement 10)
- [x] Regular testing (via `fabricks validate`)
- [x] Security policies documented

## Next Steps

- Review [Networking docs](../../docs/networking.md) for network isolation details
- Review [Capabilities docs](../../docs/capabilities.md) for security model
- Review [Production docs](../../docs/production.md) for deployment best practices
