FROM rust:1-bookworm AS builder

WORKDIR /workspace

COPY . .

RUN cargo build --release -p api-server

FROM python:3.12-slim

WORKDIR /srv/api
ENV PYTHONDONTWRITEBYTECODE=1
ENV PYTHONUNBUFFERED=1

COPY --from=builder /workspace/target/release/api-server /usr/local/bin/api-server

RUN printf '%s\n' 'grid-binance api placeholder' > /srv/api/index.html \
    && printf '%s\n' '{"status":"ok","service":"api-server"}' > /srv/api/healthz

EXPOSE 8080

HEALTHCHECK --interval=10s --timeout=3s --start-period=5s --retries=5 CMD python -c "import urllib.request; urllib.request.urlopen('http://127.0.0.1:8080/healthz')"

CMD ["sh", "-lc", "/usr/local/bin/api-server > /var/log/api-bootstrap.log 2>&1; exec python -m http.server 8080 --directory /srv/api --bind 0.0.0.0"]
