# syntax=docker/dockerfile:1

FROM rust:bookworm AS builder
WORKDIR /build
COPY Cargo.toml ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
RUN useradd -r -s /bin/false kernlreg
WORKDIR /app
COPY --from=builder /build/target/release/kernl-registry /usr/local/bin/kernl-registry
ENV KERNL_REGISTRY_PORT=9955 \
    KERNL_REGISTRY_DATA=/data/registry
RUN mkdir -p /data/registry && chown -R kernlreg:kernlreg /data
USER kernlreg
EXPOSE 9955
VOLUME ["/data/registry"]
ENTRYPOINT ["/usr/local/bin/kernl-registry"]
