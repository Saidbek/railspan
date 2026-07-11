# syntax=docker/dockerfile:1
FROM rust:bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.toml
COPY crates crates
RUN cargo build -p railspan-cli --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/railspan /usr/local/bin/railspan
ENV RAILSPAN_INGEST_ADDR=0.0.0.0:7421
EXPOSE 7421
ENTRYPOINT ["railspan"]
CMD ["serve"]
