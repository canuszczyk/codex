FROM ubuntu:24.04

ARG DEBIAN_FRONTEND=noninteractive

# Enable 'universe' and install build deps
RUN apt-get update && \
    apt-get install -y --no-install-recommends software-properties-common && \
    add-apt-repository --yes universe && \
    apt-get update && \
    apt-get install -y --no-install-recommends \
        build-essential \
        curl \
        git \
        ca-certificates \
        pkg-config \
        clang \
        musl-tools \
        libssl-dev \
        just \
        sudo && \
    rm -rf /var/lib/apt/lists/*

# Stay as root; devcontainers will attach as root by default
WORKDIR /workspace

# Install Rust + musl target as root
RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal && \
    ~/.cargo/bin/rustup target add aarch64-unknown-linux-musl && \
    ~/.cargo/bin/rustup component add clippy rustfmt

# Make cargo/rustup available in PATH
ENV PATH="/root/.cargo/bin:${PATH}"
