FROM rust:slim-buster as builder
WORKDIR /code

ENV SQLX_OFFLINE=1
COPY . .
RUN cargo b --release \
    && strip target/release/rss-qb

# 
FROM debian:buster-slim
WORKDIR /app
COPY --from=builder /code/target/release/rss-qb .
ENTRYPOINT [ "./rss-qb" ]
CMD []
