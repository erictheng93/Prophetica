FROM rust:1.85-bookworm AS builder
WORKDIR /repo
COPY . .
RUN cargo build --release --locked -p mock-sumsub

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates wget && rm -rf /var/lib/apt/lists/*
COPY --from=builder /repo/target/release/mock-sumsub /usr/local/bin/mock-sumsub
EXPOSE 8081
CMD ["mock-sumsub"]
