FROM ubuntu:16.04
MAINTAINER Yong Wen Chua <me@yongwen.xyz>
ENV PATH "/root/.cargo/bin:${PATH}"

ARG RUST_VERSION=1.19.0
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
    && apt-get remove -y --auto-remove curl \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

WORKDIR /app/src
COPY Cargo.toml Cargo.lock ./
RUN cargo fetch --locked -v

COPY ./ ./
RUN cargo build --release -v --frozen

CMD ["/app/src/target/release/rcanary"]
