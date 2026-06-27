FROM rust:stable-bookworm AS builder
WORKDIR /repo
COPY . .
RUN cargo build --release --locked -p mock-provider

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates wget && rm -rf /var/lib/apt/lists/*
COPY --from=builder /repo/target/release/mock-provider /usr/local/bin/mock-provider
EXPOSE 8081
CMD ["mock-provider"]
