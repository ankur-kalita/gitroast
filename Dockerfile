# ---- Stage 1: build ---------------------------------------------------------
# The Rust toolchain is ~1.5GB; we only want the binary, so this stage is
# thrown away after the build.
FROM rust:1-slim-bookworm AS builder

WORKDIR /src

# Build dependencies first, in their own layer, so editing src/ does not
# recompile every crate.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs \
    && cargo build --release --locked \
    && rm -rf src

COPY src ./src
# Stamped into the binary and exposed as gitroast_build_info{commit=...},
# so a running pod can be traced back to the commit it came from.
ARG GIT_SHA=unknown
ENV GIT_SHA=${GIT_SHA}
RUN touch src/main.rs && cargo build --release --locked

# ---- Stage 2: run -----------------------------------------------------------
# Distroless: no shell, no package manager, nothing to exploit but our binary.
# ureq bundles its own CA roots, so no ca-certificates are needed. The
# :nonroot tag runs as uid 65532.
FROM gcr.io/distroless/cc-debian12:nonroot

COPY --from=builder /src/target/release/gitroast /usr/local/bin/gitroast

USER 65532:65532
EXPOSE 8080

ENTRYPOINT ["/usr/local/bin/gitroast"]
CMD ["serve"]
