# Build context is the repository root (the Cargo workspace);
# compose invokes this as: context ".." dockerfile "core/ficina-jmap/Dockerfile".
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
# Also build identityctl (the admin/provisioning CLI) so the running image
# can bootstrap the first admin via `docker compose exec`.
RUN cargo build --release --package ficina-jmap --package ficina-identity --bins

FROM debian:bookworm-slim AS runtime
RUN useradd --system --no-create-home ficina \
    && mkdir -p /var/lib/ficina/blobs \
    && chown -R ficina /var/lib/ficina
COPY --from=build /src/target/release/ficina-jmap /usr/local/bin/ficina-jmap
COPY --from=build /src/target/release/identityctl /usr/local/bin/identityctl
USER ficina
ENV FICINA_BLOB_DIR=/var/lib/ficina/blobs \
    FICINA_JMAP_ADDR=0.0.0.0:8080
# The JMAP API + OIDC provider listens on plain HTTP internally; the front
# proxy (Caddy) terminates TLS on 443 and forwards here.
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/ficina-jmap"]
