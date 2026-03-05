# Azure MVP Deployment Checklist

## Resource group and region

- Create one production resource group in the selected Azure region.
- Enable zone-redundant options where supported.

## Core resources

1. Azure Container Registry (ACR)
2. Azure Container Apps Environment
3. Container App: `teams-adapter-ts` (public ingress)
4. Container App: `agent-core-rust` (internal ingress)
5. Azure Application Gateway WAF v2
6. Azure Database for PostgreSQL Flexible Server (HA)
7. Azure Key Vault
8. Azure Service Bus namespace + queue/topic
9. Application Insights + Log Analytics workspace

## Identity setup

- Entra App Registration for Teams bot + Graph delegated permissions.
- Managed Identity for both container apps.
- Key Vault access policies for managed identities.

## Required Graph permissions

- `Calendars.ReadWrite`
- `Mail.Read` or `Mail.ReadWrite`
- `Mail.Send` (only for approval-gated send flow)

## TS adapter environment variables

- `BOT_APP_ID`
- `BOT_APP_PASSWORD` (or managed identity path)
- `RUST_CORE_GRPC_URL`
- `APPINSIGHTS_CONNECTION_STRING`
- `KEY_VAULT_URI`
- `SERVICEBUS_CONNECTION_MODE`

## Rust core environment variables

- `GRPC_BIND_ADDR`
- `POSTGRES_URL`
- `KEY_VAULT_URI`
- `GRAPH_TENANT_ID`
- `GRAPH_CLIENT_ID`
- `GRAPH_CLIENT_SECRET_REF`
- `AZURE_OPENAI_ENDPOINT`
- `AZURE_OPENAI_DEPLOYMENT`
- `SERVICEBUS_NAMESPACE`
- `APPINSIGHTS_CONNECTION_STRING`

## Bot and webhook endpoints

- Bot callback endpoint hosted on TS adapter public URL.
- Graph webhook endpoint hosted on TS adapter public URL.
- Internal gRPC endpoint from TS adapter to Rust core only.

## Operational checks

- Proactive messaging check: conversation reference stored after first user DM.
- Graph webhook check: validation token echo works.
- Renewal check: subscription renewal job runs before expiry.
- Approval gate check: no send/invite without explicit approval callback.
