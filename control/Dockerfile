# Build context is the repository root (the Cargo workspace);
# compose invokes this as: context "../.." dockerfile "control/Dockerfile".
FROM rust:1.90-slim AS build
WORKDIR /src
# SQLX_OFFLINE + the checked-in .sqlx query cache let sqlx's compile-time
# query checks pass without a database at build time.
ENV SQLX_OFFLINE=true
COPY Cargo.toml Cargo.lock ./
COPY .sqlx ./.sqlx
COPY core ./core
COPY control ./control
COPY migrate ./migrate
RUN cargo build --release --package ficina-control --bins

FROM debian:bookworm-slim AS runtime
RUN useradd --system --no-create-home ficina \
    && mkdir -p /var/lib/ficina/blobs \
    && chown -R ficina /var/lib/ficina
COPY --from=build /src/target/release/ficina-control /usr/local/bin/ficina-control
USER ficina
ENV FICINA_BLOB_DIR=/var/lib/ficina/blobs \
    FICINA_CONTROL_ADDR=0.0.0.0:8090
# The control plane listens on plain HTTP internally; the front proxy (Caddy)
# terminates TLS on 443 and forwards the /control/* paths here.
EXPOSE 8090
ENTRYPOINT ["/usr/local/bin/ficina-control"]
