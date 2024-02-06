######################################################################
# https://github.com/clux/muslrust
FROM clux/muslrust:stable AS builder
COPY src ./src
COPY Cargo.* .
# https://github.com/kube-rs/version-rs/blob/main/Dockerfile
RUN --mount=type=cache,target=/volume/target \
    --mount=type=cache,target=/root/.cargo/registry \
    cargo build --release && \
    mv /volume/target/x86_64-unknown-linux-musl/release/odss2dash . && \
    mkdir -p /public

######################################################################
# https://github.com/GoogleContainerTools/distroless/blob/main/base/README.md
FROM gcr.io/distroless/static
# gcr.io/distroless/static => results in 18.3MB image
# gcr.io/distroless/base   => results in 36.3MB image

COPY --from=builder --chown=nonroot:nonroot /volume/odss2dash /app/
EXPOSE 3033
USER 10001
WORKDIR /public
ENV RUST_LOG=""
ENV RUST_BACKTRACE=0
STOPSIGNAL SIGINT
ENTRYPOINT ["/app/odss2dash"]
