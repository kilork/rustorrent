FROM alpine

VOLUME ["/app"]

RUN mkdir /download

WORKDIR /download

CMD /app/rustorrent -vvv /data/linux-5.1.16.tar.xz.torrent