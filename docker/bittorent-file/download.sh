#!/usr/bin/env bash

if [[ ! -f linux-5.1.16.tar.xz ]]; then
    curl -v https://cdn.kernel.org/pub/linux/kernel/v5.x/linux-5.1.16.tar.xz -o linux-5.1.16.tar.xz
    mktorrent -a http://bttracker:8000/announce linux-5.1.16.tar.xz
fi

if [[ ! -f /data/qbittorrent/linux-5.1.16.tar.xz ]]; then
    mkdir -p /data/qbittorrent
    cp linux-5.1.16.tar.xz /data/qbittorrent/
fi

while true; do
    if [[ ! -f linux-5.1.16.tar.xz.torrent ]]; then
        mktorrent -a http://bttracker:8000/announce linux-5.1.16.tar.xz
    fi
    if [[ ! -f /data/watch/linux-5.1.16.tar.xz.torrent ]]; then
        mkdir -p /data/watch
        cp /data/linux-5.1.16.tar.xz.torrent /data/watch/
    fi
    sleep 5
done