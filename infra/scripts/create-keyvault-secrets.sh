#!/usr/bin/env bash
set -euo pipefail

: "${KEYVAULT_NAME:?Set KEYVAULT_NAME}"
: "${BOT_APP_PASSWORD:?Set BOT_APP_PASSWORD}"
: "${AZURE_OPENAI_API_KEY:?Set AZURE_OPENAI_API_KEY}"
: "${DATABASE_URL:?Set DATABASE_URL}"

az keyvault secret set --vault-name "$KEYVAULT_NAME" --name "bot-app-password" --value "$BOT_APP_PASSWORD"
az keyvault secret set --vault-name "$KEYVAULT_NAME" --name "azure-openai-api-key" --value "$AZURE_OPENAI_API_KEY"
az keyvault secret set --vault-name "$KEYVAULT_NAME" --name "database-url" --value "$DATABASE_URL"
