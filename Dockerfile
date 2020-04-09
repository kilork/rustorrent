FROM clux/muslrust:latest as builder
WORKDIR /usr/src/
COPY . .
RUN RUSTFLAGS=-Clinker=musl-gcc PKG_CONFIG_ALLOW_CROSS=1 cargo install --path backend --target=x86_64-unknown-linux-musl

FROM alpine:latest
ENV RSBT_UI_HOST=http://localhost:8080
ENV RSBT_BIND=0.0.0.0:8080
VOLUME [ "/root/.rsbt" ]
COPY --from=builder /root/.cargo/bin/rsbt /usr/local/bin/rsbt
EXPOSE 8080
CMD rsbt --local