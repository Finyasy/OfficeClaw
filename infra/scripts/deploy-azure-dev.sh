#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

: "${AZURE_SUBSCRIPTION_ID:?Set AZURE_SUBSCRIPTION_ID}"
: "${AZURE_LOCATION:?Set AZURE_LOCATION}"
: "${AZURE_RESOURCE_GROUP:?Set AZURE_RESOURCE_GROUP}"
: "${AZURE_ENV_NAME:?Set AZURE_ENV_NAME}"
: "${BOT_APP_ID:?Set BOT_APP_ID}"
: "${BOT_APP_PASSWORD:?Set BOT_APP_PASSWORD}"
: "${BOT_TENANT_ID:=botframework.com}"
: "${POSTGRES_ADMIN_USERNAME:?Set POSTGRES_ADMIN_USERNAME}"
: "${POSTGRES_ADMIN_PASSWORD:?Set POSTGRES_ADMIN_PASSWORD}"

IMAGE_TAG="${IMAGE_TAG:-$(git -C "$ROOT_DIR" rev-parse --short HEAD)}"
POSTGRES_DB_NAME="${POSTGRES_DB_NAME:-teamsagent}"
KEYVAULT_KEK_NAME="${KEYVAULT_KEK_NAME:-teams-agent-kek}"

ACR_NAME="${ACR_NAME:-${AZURE_ENV_NAME//-/}acr}"
LOG_NAME="${LOG_NAME:-${AZURE_ENV_NAME}-log}"
APPINSIGHTS_NAME="${APPINSIGHTS_NAME:-${AZURE_ENV_NAME}-appi}"
CAE_NAME="${CAE_NAME:-${AZURE_ENV_NAME}-cae}"
KEYVAULT_NAME="${KEYVAULT_NAME:-${AZURE_ENV_NAME}-kv}"
POSTGRES_SERVER_NAME="${POSTGRES_SERVER_NAME:-${AZURE_ENV_NAME}-pg}"
POSTGRES_SERVER_NAME="${POSTGRES_SERVER_NAME//_/-}"
POSTGRES_SERVER_NAME="${POSTGRES_SERVER_NAME,,}"
AGENT_CORE_APP_NAME="${AGENT_CORE_APP_NAME:-${AZURE_ENV_NAME}-agent-core}"
TEAMS_ADAPTER_APP_NAME="${TEAMS_ADAPTER_APP_NAME:-${AZURE_ENV_NAME}-teams-adapter}"

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "missing required command: $1" >&2
    exit 1
  }
}

require_cmd az
require_cmd git

echo "Selecting subscription ${AZURE_SUBSCRIPTION_ID}"
az account set --subscription "$AZURE_SUBSCRIPTION_ID"

echo "Creating resource group ${AZURE_RESOURCE_GROUP}"
az group create \
  --name "$AZURE_RESOURCE_GROUP" \
  --location "$AZURE_LOCATION" \
  --output none

echo "Creating Azure Container Registry ${ACR_NAME}"
if ! az acr show --resource-group "$AZURE_RESOURCE_GROUP" --name "$ACR_NAME" --output none 2>/dev/null; then
  az acr create \
    --resource-group "$AZURE_RESOURCE_GROUP" \
    --name "$ACR_NAME" \
    --sku Basic \
    --admin-enabled true \
    --output none
fi

ACR_LOGIN_SERVER="$(az acr show --resource-group "$AZURE_RESOURCE_GROUP" --name "$ACR_NAME" --query loginServer -o tsv)"
ACR_USERNAME="$(az acr credential show --name "$ACR_NAME" --query username -o tsv)"
ACR_PASSWORD="$(az acr credential show --name "$ACR_NAME" --query passwords[0].value -o tsv)"

echo "Creating Log Analytics workspace ${LOG_NAME}"
if ! az monitor log-analytics workspace show --resource-group "$AZURE_RESOURCE_GROUP" --workspace-name "$LOG_NAME" --output none 2>/dev/null; then
  az monitor log-analytics workspace create \
    --resource-group "$AZURE_RESOURCE_GROUP" \
    --workspace-name "$LOG_NAME" \
    --location "$AZURE_LOCATION" \
    --output none
fi

WORKSPACE_ID="$(az monitor log-analytics workspace show --resource-group "$AZURE_RESOURCE_GROUP" --workspace-name "$LOG_NAME" --query customerId -o tsv)"
WORKSPACE_KEY="$(az monitor log-analytics workspace get-shared-keys --resource-group "$AZURE_RESOURCE_GROUP" --workspace-name "$LOG_NAME" --query primarySharedKey -o tsv)"

echo "Creating Application Insights ${APPINSIGHTS_NAME}"
if ! az monitor app-insights component show --app "$APPINSIGHTS_NAME" --resource-group "$AZURE_RESOURCE_GROUP" --output none 2>/dev/null; then
  az monitor app-insights component create \
    --app "$APPINSIGHTS_NAME" \
    --location "$AZURE_LOCATION" \
    --resource-group "$AZURE_RESOURCE_GROUP" \
    --workspace "$WORKSPACE_ID" \
    --kind web \
    --application-type web \
    --output none
fi

APPINSIGHTS_CONNECTION_STRING="$(az monitor app-insights component show --app "$APPINSIGHTS_NAME" --resource-group "$AZURE_RESOURCE_GROUP" --query connectionString -o tsv)"

echo "Creating Container Apps environment ${CAE_NAME}"
if ! az containerapp env show --name "$CAE_NAME" --resource-group "$AZURE_RESOURCE_GROUP" --output none 2>/dev/null; then
  az containerapp env create \
    --name "$CAE_NAME" \
    --resource-group "$AZURE_RESOURCE_GROUP" \
    --location "$AZURE_LOCATION" \
    --logs-workspace-id "$WORKSPACE_ID" \
    --logs-workspace-key "$WORKSPACE_KEY" \
    --output none
fi

echo "Creating Key Vault ${KEYVAULT_NAME}"
if ! az keyvault show --name "$KEYVAULT_NAME" --resource-group "$AZURE_RESOURCE_GROUP" --output none 2>/dev/null; then
  az keyvault create \
    --name "$KEYVAULT_NAME" \
    --resource-group "$AZURE_RESOURCE_GROUP" \
    --location "$AZURE_LOCATION" \
    --enable-rbac-authorization true \
    --output none
fi

if ! az keyvault key show --vault-name "$KEYVAULT_NAME" --name "$KEYVAULT_KEK_NAME" --output none 2>/dev/null; then
  az keyvault key create \
    --vault-name "$KEYVAULT_NAME" \
    --name "$KEYVAULT_KEK_NAME" \
    --kty RSA \
    --size 2048 \
    --output none
fi

KEYVAULT_URI="$(az keyvault show --name "$KEYVAULT_NAME" --resource-group "$AZURE_RESOURCE_GROUP" --query properties.vaultUri -o tsv)"
KEYVAULT_ID="$(az keyvault show --name "$KEYVAULT_NAME" --resource-group "$AZURE_RESOURCE_GROUP" --query id -o tsv)"

echo "Creating PostgreSQL flexible server ${POSTGRES_SERVER_NAME}"
if ! az postgres flexible-server show --resource-group "$AZURE_RESOURCE_GROUP" --name "$POSTGRES_SERVER_NAME" --output none 2>/dev/null; then
  az postgres flexible-server create \
    --resource-group "$AZURE_RESOURCE_GROUP" \
    --name "$POSTGRES_SERVER_NAME" \
    --location "$AZURE_LOCATION" \
    --admin-user "$POSTGRES_ADMIN_USERNAME" \
    --admin-password "$POSTGRES_ADMIN_PASSWORD" \
    --database-name "$POSTGRES_DB_NAME" \
    --sku-name Standard_B1ms \
    --tier Burstable \
    --storage-size 32 \
    --version 16 \
    --public-access 0.0.0.0 \
    --output none
fi

POSTGRES_HOST="$(az postgres flexible-server show --resource-group "$AZURE_RESOURCE_GROUP" --name "$POSTGRES_SERVER_NAME" --query fullyQualifiedDomainName -o tsv)"
DATABASE_URL="postgres://${POSTGRES_ADMIN_USERNAME}:${POSTGRES_ADMIN_PASSWORD}@${POSTGRES_HOST}:5432/${POSTGRES_DB_NAME}?sslmode=require"

echo "Building and pushing agent-core image ${IMAGE_TAG}"
az acr build \
  --registry "$ACR_NAME" \
  --image "agent-core:${IMAGE_TAG}" \
  --file "$ROOT_DIR/apps/agent-core/Dockerfile" \
  "$ROOT_DIR" \
  --output none

echo "Building and pushing teams-adapter image ${IMAGE_TAG}"
az acr build \
  --registry "$ACR_NAME" \
  --image "teams-adapter:${IMAGE_TAG}" \
  --file "$ROOT_DIR/apps/teams-adapter/Dockerfile" \
  "$ROOT_DIR" \
  --output none

AGENT_CORE_IMAGE="${ACR_LOGIN_SERVER}/agent-core:${IMAGE_TAG}"
TEAMS_ADAPTER_IMAGE="${ACR_LOGIN_SERVER}/teams-adapter:${IMAGE_TAG}"

echo "Deploying agent-core container app ${AGENT_CORE_APP_NAME}"
az containerapp create \
  --name "$AGENT_CORE_APP_NAME" \
  --resource-group "$AZURE_RESOURCE_GROUP" \
  --environment "$CAE_NAME" \
  --image "$AGENT_CORE_IMAGE" \
  --ingress internal \
  --target-port 50051 \
  --transport http2 \
  --min-replicas 1 \
  --max-replicas 2 \
  --registry-server "$ACR_LOGIN_SERVER" \
  --registry-username "$ACR_USERNAME" \
  --registry-password "$ACR_PASSWORD" \
  --system-assigned \
  --secrets database-url="$DATABASE_URL" \
  --env-vars \
    GRPC_PORT=50051 \
    DATABASE_URL=secretref:database-url \
    KEYVAULT_URI="$KEYVAULT_URI" \
    KEYVAULT_KEK_NAME="$KEYVAULT_KEK_NAME" \
    APPINSIGHTS_CONNECTION_STRING="$APPINSIGHTS_CONNECTION_STRING" \
  --query properties.configuration.ingress.fqdn \
  --output tsv >/tmp/officeclaw-agent-core-fqdn.txt

AGENT_CORE_FQDN="$(cat /tmp/officeclaw-agent-core-fqdn.txt)"
AGENT_GRPC_ENDPOINT="grpcs://${AGENT_CORE_FQDN}:443"

echo "Deploying teams-adapter container app ${TEAMS_ADAPTER_APP_NAME}"
az containerapp create \
  --name "$TEAMS_ADAPTER_APP_NAME" \
  --resource-group "$AZURE_RESOURCE_GROUP" \
  --environment "$CAE_NAME" \
  --image "$TEAMS_ADAPTER_IMAGE" \
  --ingress external \
  --target-port 3978 \
  --transport http \
  --min-replicas 1 \
  --max-replicas 2 \
  --registry-server "$ACR_LOGIN_SERVER" \
  --registry-username "$ACR_USERNAME" \
  --registry-password "$ACR_PASSWORD" \
  --secrets bot-app-password="$BOT_APP_PASSWORD" \
  --env-vars \
    PORT=3978 \
    BOT_APP_ID="$BOT_APP_ID" \
    BOT_APP_PASSWORD=secretref:bot-app-password \
    BOT_TENANT_ID="$BOT_TENANT_ID" \
    BOT_TOKEN_SCOPE="https://api.botframework.com/.default" \
    AGENT_GRPC_ENDPOINT="$AGENT_GRPC_ENDPOINT" \
    APPINSIGHTS_CONNECTION_STRING="$APPINSIGHTS_CONNECTION_STRING" \
  --query properties.configuration.ingress.fqdn \
  --output tsv >/tmp/officeclaw-teams-adapter-fqdn.txt

TEAMS_ADAPTER_FQDN="$(cat /tmp/officeclaw-teams-adapter-fqdn.txt)"
TEAMS_ADAPTER_BASE_URL="https://${TEAMS_ADAPTER_FQDN}"

echo "Updating agent-core with adapter base URL"
az containerapp update \
  --name "$AGENT_CORE_APP_NAME" \
  --resource-group "$AZURE_RESOURCE_GROUP" \
  --set-env-vars \
    GRPC_PORT=50051 \
    DATABASE_URL=secretref:database-url \
    KEYVAULT_URI="$KEYVAULT_URI" \
    KEYVAULT_KEK_NAME="$KEYVAULT_KEK_NAME" \
    TEAMS_ADAPTER_BASE_URL="$TEAMS_ADAPTER_BASE_URL" \
    APPINSIGHTS_CONNECTION_STRING="$APPINSIGHTS_CONNECTION_STRING" \
  --output none

AGENT_CORE_PRINCIPAL_ID="$(az containerapp show --name "$AGENT_CORE_APP_NAME" --resource-group "$AZURE_RESOURCE_GROUP" --query identity.principalId -o tsv)"

if [[ -n "$AGENT_CORE_PRINCIPAL_ID" ]]; then
  echo "Granting Key Vault Crypto User to agent-core managed identity"
  az role assignment create \
    --assignee-object-id "$AGENT_CORE_PRINCIPAL_ID" \
    --assignee-principal-type ServicePrincipal \
    --role "Key Vault Crypto User" \
    --scope "$KEYVAULT_ID" \
    --output none || true
fi

cat <<EOF
Deployment complete.

Adapter URL: ${TEAMS_ADAPTER_BASE_URL}
Agent core gRPC endpoint: ${AGENT_GRPC_ENDPOINT}
Key Vault URI: ${KEYVAULT_URI}
Postgres host: ${POSTGRES_HOST}

Next step:
  GRAPH_ACCESS_TOKEN=... TENANT_ID=... USER_ID=... ADAPTER_BASE_URL=${TEAMS_ADAPTER_BASE_URL} \
  ${ROOT_DIR}/infra/scripts/smoke-unread-summary.sh
EOF
