# syntax=docker/dockerfile:1
# Headless anabios: the deterministic simulation core and its CLI, with no
# Godot, no GPU and no network needed at run time.
#
#   docker build -t anabios .
#   docker run --rm anabios                                   # 1000 ticks of scenarios/minimal.toml
#   docker run --rm anabios run --scenario scenarios/tribes.toml --ticks 5000
#   docker run --rm -v "$PWD/runs:/runs" anabios sweep \
#       --scenario scenarios/speciation.toml --seeds 8 --ticks 5000 --out /runs/speciation-8
#
# The run is bit-identical to a native build of the same commit: compare the
# `state_hash=` the last line prints against `cargo run --release -p
# anabios-headless -- run --scenario scenarios/minimal.toml --ticks 1000`.

FROM rust:1-slim-bookworm AS builder
WORKDIR /src
# Cargo.lock pins every dependency; --locked refuses to build if it would drift.
# rust-toolchain.toml is deliberately not copied: it asks rustup for "stable",
# which the image already provides, and copying it would trigger a toolchain
# download at build time.
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
# A few core modules embed scenario files with include_str!, so the builder
# needs them too, not only the runtime image.
COPY scenarios/ scenarios/
RUN cargo build --release --locked -p anabios-headless

FROM debian:bookworm-slim
WORKDIR /app
COPY --from=builder /src/target/release/anabios-headless /usr/local/bin/anabios-headless
COPY scenarios/ scenarios/
# Sweeps and recordings write under --out; mount a host directory there.
VOLUME ["/runs"]
ENTRYPOINT ["anabios-headless"]
CMD ["run", "--scenario", "scenarios/minimal.toml", "--ticks", "1000"]
