FROM rust:1-bookworm AS builder

WORKDIR /workspace

COPY . .

ARG APP_NAME
RUN cargo build --release -p "${APP_NAME}"

FROM python:3.12-slim

WORKDIR /srv/app
ENV PYTHONDONTWRITEBYTECODE=1
ENV PYTHONUNBUFFERED=1

ARG APP_NAME
ENV APP_NAME=${APP_NAME}

COPY --from=builder /workspace/target/release/${APP_NAME} /usr/local/bin/${APP_NAME}

CMD ["sh", "-lc", "exec /usr/local/bin/$APP_NAME"]
