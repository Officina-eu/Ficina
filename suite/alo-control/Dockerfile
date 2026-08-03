# Build context is the repository root (the Cargo workspace);
# compose invokes this as: context "../.." dockerfile "control/Dockerfile".
FROM rust:1.90-slim AS build
WORKDIR /src
# SQLX_OFFLINE + the checked-in .sqlx query cache let sqlx's compile-time
# query checks pass without a database at build time.
ENV SQLX_OFFLINE=true
COPY Cargo.toml Cargo.lock ./
COPY .sqlx ./.sqlx
COPY platform ./platform
COPY products ./products
COPY suite ./suite
COPY migrate ./migrate
RUN cargo build --release --package alo-control --bins

FROM debian:bookworm-slim AS runtime
RUN groupadd --system --gid 10001 alo && useradd --system --no-create-home --uid 10001 --gid 10001 alo \
    && mkdir -p /var/lib/alo/blobs \
    && chown -R alo /var/lib/alo
COPY --from=build /src/target/release/alo-control /usr/local/bin/alo-control
USER alo
ENV ALO_BLOB_DIR=/var/lib/alo/blobs \
    ALO_CONTROL_ADDR=0.0.0.0:8090
# The control plane listens on plain HTTP internally; the front proxy (Caddy)
# terminates TLS on 443 and forwards the /control/* paths here.
EXPOSE 8090
ENTRYPOINT ["/usr/local/bin/alo-control"]
