# Azure Dev Deployment and Live Unread Smoke Test

This is the first live-system validation path for OfficeClaw.

It intentionally tests the deployed adapter and Rust core with a real Microsoft Graph token before adding the full Teams install/auth loop.

## Why this smoke test comes first

- It exercises the deployed stack end to end: HTTP adapter -> gRPC -> Rust core -> Postgres -> Graph.
- It keeps the first live validation read-only.
- It isolates Azure/container/runtime issues before Bot Framework and Teams UX issues are introduced.

## Preconditions

- Azure CLI installed and authenticated.
- Docker access is not required locally because images are built with `az acr build`.
- Entra app registration exists for the bot if you want to progress to real Teams traffic later.
- A real delegated Microsoft Graph access token is available for the test user.

## Required environment variables

For deployment:

- `AZURE_SUBSCRIPTION_ID`
- `AZURE_LOCATION`
- `AZURE_RESOURCE_GROUP`
- `AZURE_ENV_NAME`
- `BOT_APP_ID`
- `BOT_APP_PASSWORD`
- `BOT_TENANT_ID` (optional; defaults to `botframework.com`)
- `POSTGRES_ADMIN_USERNAME`
- `POSTGRES_ADMIN_PASSWORD`

For the live unread smoke test:

- `ADAPTER_BASE_URL`
- `GRAPH_ACCESS_TOKEN`
- `TENANT_ID`
- `USER_ID`
- `USER_DISPLAY_NAME` (optional)

## Deployment command

Set the variables from [infra/.env.dev.example](/Users/bryan.bosire/anaconda_projects/OfficeClaw/infra/.env.dev.example), then run:

```bash
infra/scripts/deploy-azure-dev.sh
```

The script performs these steps:

1. Creates the Azure resource group.
2. Creates ACR, Log Analytics, Application Insights, Container Apps Environment, Key Vault, and PostgreSQL.
3. Builds and pushes the two app images with `az acr build`.
4. Deploys `agent-core` with internal gRPC ingress.
5. Deploys `teams-adapter` with external HTTP ingress.
6. Updates `agent-core` with the adapter base URL for proactive callbacks.
7. Grants the `Key Vault Crypto User` role to the `agent-core` managed identity.

## Live unread smoke test command

```bash
ADAPTER_BASE_URL=https://<adapter-fqdn> \
GRAPH_ACCESS_TOKEN=<graph-token> \
TENANT_ID=<tenant-id> \
USER_ID=<aad-user-id> \
infra/scripts/smoke-unread-summary.sh
```

## Expected result

- `/oauth/callback` stores the delegated Graph token in Rust-owned storage.
- `/api/messages` returns an unread-email summary response.
- The request is audited and the session state is persisted in PostgreSQL.

## What this does not prove yet

- Real Teams ingress and Bot Framework auth.
- Live Adaptive Card callbacks from Teams.
- Approval-gated send or invite execution.
- Graph webhook subscriptions and proactive Teams delivery.

Those come after this smoke path is green.
