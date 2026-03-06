# teams-adapter (TypeScript)

Thin Teams/Bot edge adapter.

## Included

- Activity normalization (`src/bot/bot.ts`)
- Config validation (`src/config.ts`)
- Conversation reference store (`src/storage/conversation_refs.ts`)
- Real gRPC transport client (`src/transport/agent_grpc.ts`)
- Unit tests in `tests/`

## Run

```bash
pnpm install --frozen-lockfile
pnpm test
```

## Container build

```bash
docker build -f apps/teams-adapter/Dockerfile .
```
