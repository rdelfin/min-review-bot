ARG ARCH=
FROM ${ARCH}lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM ${ARCH}chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json
# Build application
COPY . .
COPY min.env .env
RUN cargo build --release --bin daemon

FROM ${ARCH}debian:buster-slim
COPY --from=builder /app/target/release/daemon /usr/local/bin/min_review_daemon

RUN apt-get update \
    && apt-get install -y libssl1.1 sqlite3 \
    && apt-get autoremove \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists

CMD ["min_review_daemon"]
