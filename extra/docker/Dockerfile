FROM rust:latest as builder
WORKDIR /usr/src/myapp
COPY . .
RUN cargo install --path rsbt

FROM debian:buster-slim
COPY --from=builder /usr/local/cargo/bin/rsbt /usr/local/bin/rsbt
CMD sleep 30 && rsbt -vvv /data/linux-5.1.16.tar.xz.torrent
