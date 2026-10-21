ARG RUNTIME_IMAGE=rust:1.89-slim-bookworm
FROM rust:1.89-slim-bookworm AS build
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY maintainer.example.yml ./
RUN cargo build --release --locked

FROM ${RUNTIME_IMAGE}
ARG INSTALL_CODEX=true
RUN apt-get update && apt-get install -y --no-install-recommends \
    git curl ca-certificates build-essential pkg-config \
    && rm -rf /var/lib/apt/lists/* \
    && if command -v rustup >/dev/null; then \
         rustup toolchain install stable --profile minimal --component rustfmt --component clippy \
         && rustup default stable; fi \
    && useradd --create-home --uid 10001 maintainer
COPY --from=build /build/target/release/repo-maintainer /usr/local/bin/repo-maintainer
COPY scripts/install-codex.sh /usr/local/lib/maintainer/install-codex.sh
RUN if [ "$INSTALL_CODEX" = true ]; then bash /usr/local/lib/maintainer/install-codex.sh; \
    elif [ "$INSTALL_CODEX" != false ]; then echo 'INSTALL_CODEX must be true or false' >&2; exit 1; fi
ENV CARGO_HOME=/tmp/maintainer-cargo HOME=/tmp/maintainer-home
USER maintainer
WORKDIR /repository
ENTRYPOINT ["repo-maintainer"]
CMD ["check", "--repo", "/repository"]
