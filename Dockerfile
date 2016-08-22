FROM lawliet89/debian-rust:1.10.0

COPY Cargo.toml Cargo.lock ./
RUN cargo fetch

COPY . ./
RUN cargo build --release

VOLUME /app/src/config

ENTRYPOINT ["cargo"]
CMD ["run", "--release", "--", "/app/src/config/config.toml"]
