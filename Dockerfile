FROM node:24-bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    python3 python3-venv git curl ca-certificates build-essential pkg-config ripgrep \
    && rm -rf /var/lib/apt/lists/*

ENV VIRTUAL_ENV=/opt/venv \
    CARGO_HOME=/opt/cargo \
    RUSTUP_HOME=/opt/rustup \
    RUSTUP_TOOLCHAIN=stable
ENV PATH="/opt/venv/bin:/opt/cargo/bin:${PATH}"
WORKDIR /opt/maintainer
COPY requirements.txt ./
RUN python3 -m venv /opt/venv && pip install --no-cache-dir -r requirements.txt
COPY agent/package*.json ./agent/
RUN npm ci --prefix agent --ignore-scripts --no-audit --no-fund
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o /tmp/rustup.sh \
    && sh /tmp/rustup.sh -y --profile minimal --default-toolchain stable --component rustfmt --component clippy \
    && rm /tmp/rustup.sh
COPY agent ./agent
COPY scripts ./scripts
COPY examples ./examples
COPY maintainer.example.yml ./

ENV CARGO_HOME=/tmp/maintainer-cargo HOME=/tmp/maintainer-home
USER node
WORKDIR /repository
ENTRYPOINT ["python3", "/opt/maintainer/scripts/cli.py"]
CMD ["check"]
