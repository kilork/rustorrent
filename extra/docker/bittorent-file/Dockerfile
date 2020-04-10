FROM alpine

RUN apk --no-cache add curl bash mktorrent

COPY download.sh /

WORKDIR /data/

VOLUME [ "/data" ]

CMD /download.sh