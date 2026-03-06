FROM node:22-bookworm-slim AS build

WORKDIR /workspace/apps/teams-adapter

RUN corepack enable

COPY apps/teams-adapter/package.json apps/teams-adapter/pnpm-lock.yaml apps/teams-adapter/tsconfig.json apps/teams-adapter/eslint.config.js ./
RUN pnpm install --frozen-lockfile

COPY proto /workspace/proto
COPY apps/teams-adapter/src ./src

RUN pnpm run build

FROM node:22-bookworm-slim

WORKDIR /app

RUN corepack enable

COPY apps/teams-adapter/package.json apps/teams-adapter/pnpm-lock.yaml ./
RUN pnpm install --prod --frozen-lockfile

COPY --from=build /workspace/apps/teams-adapter/dist ./dist
COPY --from=build /workspace/proto ./proto

EXPOSE 3978

CMD ["node", "dist/src/index.js"]
