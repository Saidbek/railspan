# syntax=docker/dockerfile:1
FROM rust:bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates crates
RUN cargo build -p railspan-cli --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
  && rm -rf /var/lib/apt/lists/* \
  && useradd -r -u 10001 railspan
COPY --from=builder /app/target/release/railspan /usr/local/bin/railspan
RUN mkdir -p /data && chown railspan:railspan /data
USER railspan
ENV RAILSPAN_INGEST_ADDR=0.0.0.0:7421
ENV RAILSPAN_DATA_DIR=/data
EXPOSE 7421
VOLUME ["/data"]
ENTRYPOINT ["railspan"]
CMD ["serve"]
