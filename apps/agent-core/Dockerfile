FROM rust:1.83-bookworm AS builder

RUN apt-get update \
  && apt-get install -y --no-install-recommends protobuf-compiler ca-certificates \
  && rm -rf /var/lib/apt/lists/*

WORKDIR /workspace/apps/agent-core

COPY proto /workspace/proto
COPY apps/agent-core/Cargo.toml apps/agent-core/Cargo.lock apps/agent-core/build.rs ./
COPY apps/agent-core/src ./src

RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates \
  && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /workspace/apps/agent-core/target/release/agent-core /usr/local/bin/agent-core

EXPOSE 50051

CMD ["agent-core"]
