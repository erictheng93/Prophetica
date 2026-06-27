FROM rust:stable-bookworm AS builder
WORKDIR /repo
COPY . .
RUN cargo build --release --locked -p mock-payment

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates wget && rm -rf /var/lib/apt/lists/*
COPY --from=builder /repo/target/release/mock-payment /usr/local/bin/mock-payment
EXPOSE 8082
CMD ["mock-payment"]
