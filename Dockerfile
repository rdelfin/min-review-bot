ARG ARCH=
FROM ${ARCH}rust:1.65-buster as builder

WORKDIR /usr/src

ADD . ./
ADD min.env .env

RUN cargo build --release

FROM ${ARCH}debian:buster-slim
COPY --from=builder /usr/src/target/release/daemon /usr/bin/min_review_daemon

RUN apt-get update \
    && apt-get install -y libssl1.1 sqlite3 \
    && apt-get autoremove \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists

CMD ["min_review_daemon"]
