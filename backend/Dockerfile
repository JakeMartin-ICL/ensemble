# Build stage
FROM rust:1.93-slim AS builder
WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
COPY . .
RUN cargo build --release --bin api

# Runtime stage
FROM debian:bookworm-slim
LABEL org.opencontainers.image.source="https://github.com/JakeMartin-ICL/ensemble"
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/api /usr/local/bin/api
EXPOSE 3000
CMD ["api"]
