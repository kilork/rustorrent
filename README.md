# torrent-rs - BitTorrent protocol client

## WORK IN PROGRESS

## Legal

Dual-licensed under MIT or the [UNLICENSE](http://unlicense.org/).

## Features

- Implements bittorrent client.

## Installation

    cargo install --features ui --path .

## Development

Install dependencies:

    cargo xtask install

Start dev Rust+Webpack server in watch mode:

    cargo xtask dev

Start only Webpack server in dev mode:

    cargo xtask ui-dev

Clean build directories:

    cargo xtask clean

## Configuration

We use [confy](https://docs.rs/confy) for configuration. Configuration is stored in `toml` format.

Locations:

| OS  | Location                                                       |
|-----|----------------------------------------------------------------|
| Mac | ~/Library/Preferences/rs.rustorrent.rustorrent/rustorrent.toml |
