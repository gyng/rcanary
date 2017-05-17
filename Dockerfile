FROM japaric/x86_64-unknown-linux-musl:v0.1.10 as builder
MAINTAINER Yong Wen Chua <me@yongwen.xyz>
ENV PATH "/root/.cargo/bin:${PATH}"

ARG RUST_VERSION=1.17.0
ARG ARCHITECTURE=x86_64-unknown-linux-musl
RUN set -x \
    && apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
                                          build-essential \
                                          ca-certificates \
                                          curl \
                                          libcurl3 \
                                          git \
                                          file \
                                          libssl-dev \
                                          pkg-config \
    && curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain ${RUST_VERSION} \
    && rustup target add "${ARCHITECTURE}" \
    && apt-get remove -y --auto-remove curl \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

WORKDIR /app/src
COPY Cargo.toml Cargo.lock ./
RUN cargo fetch --locked -v

COPY ./ ./
RUN cargo build --release --target "${ARCHITECTURE}" -v --frozen

# Runtime Image

FROM alpine:3.5
ARG ARCHITECTURE=x86_64-unknown-linux-musl
WORKDIR /app
COPY --from=builder /app/src/target/${ARCHITECTURE}/release/rcanary .
CMD [/app/rcanary]
