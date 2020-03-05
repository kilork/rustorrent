FROM rust:latest as builder
WORKDIR /usr/src/myapp
COPY . .
RUN cargo install --path rustorrent

FROM debian:buster-slim
COPY --from=builder /usr/local/cargo/bin/rustorrent /usr/local/bin/rustorrent
CMD sleep 30 && rustorrent -vvv /data/linux-5.1.16.tar.xz.torrent
