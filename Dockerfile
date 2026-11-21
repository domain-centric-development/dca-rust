# syntax=docker/dockerfile:1
# `docker build .` succeeds exactly when the crates build, the fixture tests pass, Clippy finds nothing and
# the formatting holds and the docs build without a warning - in the workspace and in the example, whose tests include the rule suite run over it.
# Fully qualified image names, so the build works under Podman without a registry alias.
FROM docker.io/library/rust:1-bookworm AS build
WORKDIR /workspace
RUN rustup component add clippy rustfmt
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo test --workspace \
 && cargo clippy --workspace --all-targets -- -D warnings \
 && cargo fmt --all --check \
 && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps \
 && cd example \
 && cargo test --workspace \
 && cargo clippy --workspace --all-targets -- -D warnings \
 && cargo fmt --all --check \
 && cd .. \
 && mkdir -p /out && cp dca-rules/rules.json /out/

FROM docker.io/library/debian:bookworm-slim
COPY --from=build /out /out
CMD ["ls", "-l", "/out"]
