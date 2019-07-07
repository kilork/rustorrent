#!/usr/bin/env bash

if [[ ! -f linux-5.1.16.tar.xz ]]; then
    curl -v https://cdn.kernel.org/pub/linux/kernel/v5.x/linux-5.1.16.tar.xz -o linux-5.1.16.tar.xz
    mktorrent -a http://bttracker:8000/announce linux-5.1.16.tar.xz
fi