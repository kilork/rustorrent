# torrent-rs - BitTorrent protocol client

## WORK IN PROGRESS

## Legal

Dual-licensed under MIT or the [UNLICENSE](http://unlicense.org/).

## Features

- Implements bittorrent client.

## Installation

CLI (not really useful):

    cargo install --path rustorrent

WEB UI (requires npm to build):

    cargo install --features ui --path rustorrent-web

## Development

Install dependencies (requires npm):

    cargo xtask install

Start dev Rust+Webpack server in watch mode:

    cargo xtask dev

Start only Webpack server in dev mode:

    cargo xtask ui-dev

Clean build directories:

    cargo xtask clean

Run rustorrent cli in Docker to check interaction with popular torrent clients:

    docker-compose up

## Configuration

We use [confy](https://docs.rs/confy) for configuration. Configuration is stored in `toml` format.

Locations:

| OS  | Location                                                       |
|-----|----------------------------------------------------------------|
| Mac | ~/Library/Preferences/rs.rustorrent.rustorrent/rustorrent.toml |
