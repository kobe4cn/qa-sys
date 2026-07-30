ARG DEV_IMAGE=qa-project-dev:v1.0
FROM ${DEV_IMAGE} AS builder

LABEL org.opencontainers.image.authors="kevin"

ENV LANG=C.UTF-8

WORKDIR /app

COPY . .

RUN cargo build --release

FROM debian:bookworm-slim

WORKDIR /app

ENV APP_CONFIG=/app/config/app.yaml
VOLUME /app/config

EXPOSE 50051
EXPOSE 2338

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        bash \
        ca-certificates \
        curl \
        net-tools \
        vim \
    && update-ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /app/config

COPY --from=builder /app/target/release/qa-svc /app/qa-svc

CMD ["/app/qa-svc"]
