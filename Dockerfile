FROM lawliet89/debian-rust:1.14.0

WORKDIR /app/src

VOLUME /app/config
VOLUME /app/logs
EXPOSE 8099

COPY entrypoint.sh /entrypoint.sh
ENTRYPOINT ["/entrypoint.sh"]
CMD ["--"]

COPY ./ ./
RUN cargo build --release
