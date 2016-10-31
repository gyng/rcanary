FROM lawliet89/debian-rust:1.12.1

COPY Cargo.toml Cargo.lock ./
RUN cargo fetch

COPY . ./
RUN cargo build --release

VOLUME /app/src/config
EXPOSE 8099

ENTRYPOINT ["cargo"]
CMD ["run", "--release", "--", "/app/src/config/config.toml"]
