# Build context is the repository root (the Cargo workspace);
# compose invokes this as: context ".." dockerfile "core/ficina-smtp/Dockerfile".
FROM rust:1.90-slim AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY core ./core
COPY control ./control
COPY migrate ./migrate
RUN cargo build --release --package ficina-smtp

FROM debian:bookworm-slim AS runtime
RUN useradd --system --no-create-home ficina \
    && mkdir -p /var/spool/ficina \
    && chown ficina /var/spool/ficina
COPY --from=build /src/target/release/ficina-smtp /usr/local/bin/ficina-smtp
USER ficina
ENV FICINA_SMTP_SPOOL_DIR=/var/spool/ficina
EXPOSE 2525
# Healthcheck is the binary itself (`--healthcheck` probes for the 220
# greeting), so the runtime image carries no shell tooling for it.
ENTRYPOINT ["/usr/local/bin/ficina-smtp"]
