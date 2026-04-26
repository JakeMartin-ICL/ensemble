# Build stage
FROM rust:1.77-slim AS builder
WORKDIR /app
COPY . .
RUN cargo build --release --bin api

# Runtime stage
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/api /usr/local/bin/api
EXPOSE 3000
CMD ["api"]
