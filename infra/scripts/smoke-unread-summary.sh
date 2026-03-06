#!/usr/bin/env bash
set -euo pipefail

: "${ADAPTER_BASE_URL:?Set ADAPTER_BASE_URL}"
: "${GRAPH_ACCESS_TOKEN:?Set GRAPH_ACCESS_TOKEN}"
: "${TENANT_ID:?Set TENANT_ID}"
: "${USER_ID:?Set USER_ID}"

USER_DISPLAY_NAME="${USER_DISPLAY_NAME:-OfficeClaw Smoke Test}"
CONVERSATION_ID="${CONVERSATION_ID:-smoke-$(date +%s)}"

oauth_payload="$(cat <<JSON
{
  "tenant_id": "${TENANT_ID}",
  "user_id": "${USER_ID}",
  "user_display_name": "${USER_DISPLAY_NAME}",
  "access_token": "${GRAPH_ACCESS_TOKEN}"
}
JSON
)"

activity_payload="$(cat <<JSON
{
  "channelId": "teams",
  "id": "smoke-msg-1",
  "serviceUrl": "${ADAPTER_BASE_URL}",
  "text": "Summarize unread emails from today",
  "from": {
    "aadObjectId": "${USER_ID}",
    "name": "${USER_DISPLAY_NAME}"
  },
  "recipient": {
    "id": "officeclaw-bot",
    "name": "OfficeClaw"
  },
  "conversation": {
    "id": "${CONVERSATION_ID}"
  },
  "channelData": {
    "tenant": {
      "id": "${TENANT_ID}"
    }
  },
  "value": {}
}
JSON
)"

echo "Storing Graph token via adapter /oauth/callback"
curl --fail --silent --show-error \
  -X POST "${ADAPTER_BASE_URL%/}/oauth/callback" \
  -H "content-type: application/json" \
  -d "${oauth_payload}" >/tmp/officeclaw-smoke-oauth.json

echo "Calling deployed adapter /api/messages"
curl --fail --silent --show-error \
  -X POST "${ADAPTER_BASE_URL%/}/api/messages" \
  -H "content-type: application/json" \
  -d "${activity_payload}" >/tmp/officeclaw-smoke-response.json

if command -v python3 >/dev/null 2>&1; then
  python3 -m json.tool /tmp/officeclaw-smoke-response.json
else
  cat /tmp/officeclaw-smoke-response.json
fi
