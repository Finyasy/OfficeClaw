FROM rust:1.83 as builder
WORKDIR /app
COPY Cargo.toml build.rs ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
WORKDIR /app
COPY --from=builder /app/target/release/agent-core /usr/local/bin/agent-core
EXPOSE 50051
CMD ["agent-core"]
