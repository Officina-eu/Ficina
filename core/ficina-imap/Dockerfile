# Build context is the repository root (the Cargo workspace);
# compose invokes this as: context ".." dockerfile "core/ficina-imap/Dockerfile".
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
RUN cargo build --release --package ficina-imap

FROM debian:bookworm-slim AS runtime
RUN useradd --system --no-create-home ficina \
    && mkdir -p /var/lib/ficina/blobs \
    && chown -R ficina /var/lib/ficina
COPY --from=build /src/target/release/ficina-imap /usr/local/bin/ficina-imap
USER ficina
ENV FICINA_BLOB_DIR=/var/lib/ficina/blobs
# 993 implicit-TLS IMAP, 143 STARTTLS IMAP, 995 implicit-TLS POP3.
EXPOSE 993 143 995
# The binary probes itself (`--healthcheck` TCP-connects a listener), so the
# runtime image needs no extra tooling.
ENTRYPOINT ["/usr/local/bin/ficina-imap"]
