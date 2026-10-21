FROM rust:1.89-slim-bookworm AS build
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY maintainer.example.yml ./
COPY renovate.example.json renovate-image.txt ./
RUN cargo build --release --locked

FROM rust:1.89-slim-bookworm
RUN apt-get update && apt-get install -y --no-install-recommends \
    git ca-certificates build-essential pkg-config \
    && rm -rf /var/lib/apt/lists/* \
    && rustup toolchain install stable --profile minimal --component rustfmt --component clippy \
    && rustup default stable \
    && useradd --create-home --uid 1000 maintainer \
    && dpkg --compare-versions \
        "$(dpkg-query --show --showformat='${Version}' ca-certificates)" ge '20250419~deb12u2'
COPY --from=build /build/target/release/repo-maintainer /usr/local/bin/repo-maintainer
ENV CARGO_HOME=/tmp/maintainer-cargo HOME=/tmp/maintainer-home
USER maintainer
WORKDIR /repository
ENTRYPOINT ["repo-maintainer"]
CMD ["check", "--repo", "/repository"]
