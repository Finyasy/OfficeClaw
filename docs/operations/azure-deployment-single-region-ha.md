# Azure Deployment Baseline (Single-Region HA)

## Selected production options

| Area | Selected option | Reason |
|---|---|---|
| Compute hosting | Azure Container Apps for `teams-adapter-ts` and `agent-core-rust` | Best balance of ops simplicity, autoscaling, and revision-based rollout for MVP to production. |
| Public entry | Azure Application Gateway (WAF v2) in front of TS adapter | Strong single-region ingress/WAF fit without multi-region overhead. |
| Service-to-service | Internal-only ingress from TS adapter to Rust core | Keeps core private and reduces attack surface. |
| Identity | Entra App Registration + Managed Identity | Delegated Graph auth for users plus secretless Azure resource access. |
| Secrets and keys | Azure Key Vault | Centralized storage for bot secret, Graph client secret, and token encryption keys. |
| Primary database | Azure Database for PostgreSQL Flexible Server (HA) | Managed relational storage for sessions, approvals, audits, and conversation refs. |
| Async decoupling | Azure Service Bus (enabled) | Reliable buffering for long agent runs and webhook-driven workloads. |
| Observability | Application Insights + Azure Monitor + Log Analytics | End-to-end tracing and alerting across adapter, core, and Graph calls. |
| CI/CD | GitHub Actions + ACR + Container Apps revisions | Controlled deployments with rollback support. |

## Deployment model

- Single Azure region with zone redundancy for core workloads.
- TS adapter and Rust core deployed as separate services.
- Managed Postgres with HA enabled.
- Secrets and token encryption keys stored in Azure Key Vault.
- Service Bus is used for asynchronous job execution and retryable background tasks.

## Networking

- Private service-to-service communication where possible.
- Ingress only through controlled endpoints.
- Egress restrictions for Graph and Azure OpenAI endpoints.
- Rust core has no public ingress; only TS adapter receives public traffic.

## Reliability

- Health probes and autoscaling for adapter and core.
- Retry/backoff policy for Graph and gRPC transient failures.
- Alerting for approval queue lag, Graph failure rates, and message delivery errors.
- Blue/green or canary via Container Apps revisions before full rollout.

## Observability

- Structured logs with correlation IDs.
- Metrics: request latency, error rate, approval turnaround.
- Traces: adapter ingress -> core orchestrator -> Graph -> response.
- Include conversation reference write/read metrics for proactive messaging health.

## Teams and Graph requirements captured

- Store Teams conversation references after first user interaction to support proactive messaging.
- Expose public HTTPS endpoints for bot callbacks and Graph webhook notifications.
- Implement webhook validation-token response and subscription renewal jobs.
