#!/usr/bin/env bash

mkdir -p /var/lib/transmission-daemon/downloads
mkdir -p /var/lib/transmission-daemon/watch
cp /data/linux-5.1.16.tar.xz /var/lib/transmission-daemon/downloads/

while [[ ! -f /data/linux-5.1.16.tar.xz.torrent ]]; do
    echo "waiting for torrent download... 5 secs"
    sleep 5
done

cp /data/linux-5.1.16.tar.xz.torrent /var/lib/transmission-daemon/watch/
/usr/bin/transmission.sh