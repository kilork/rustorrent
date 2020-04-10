FROM dperson/transmission

RUN apk --no-cache add transmission-cli tcpdump

COPY run.sh /usr/bin/
COPY settings.json /var/lib/transmission-daemon/info/settings.json

VOLUME [ "/data" ]

CMD ["/sbin/tini", "--", "/usr/bin/run.sh"]