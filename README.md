# torrent-rs - BitTorrent protocol client

## WORK IN PROGRESS

<img alt="RSBT" src="frontend/www/src/main/webapp/content/images/rsbt_mascot.svg" width="256px" height="256px">

## Legal

Source code except logo / artwork images is dual-licensed under MIT or the [UNLICENSE](http://unlicense.org/).

<a rel="license" href="http://creativecommons.org/licenses/by-nc-nd/4.0/"><img alt="Creative Commons Licence" style="border-width:0" src="https://i.creativecommons.org/l/by-nc-nd/4.0/88x31.png" /></a><br />Artwork which is not part of JHipster is licensed under a <a rel="license" href="http://creativecommons.org/licenses/by-nc-nd/4.0/">Creative Commons Attribution-NonCommercial-NoDerivatives 4.0 International License</a>.

## Features

- Implements bittorrent client.

## Installation

CLI (not really useful):

    cargo install --path cli

WEB UI (requires npm to build):

    cargo install --features ui --path backend

## Development

Read design documents: [DESIGN](DESIGN.md)

Install dependencies (requires npm):

    cargo xtask install

Start dev Rust+Webpack server in watch mode:

    cargo xtask dev

Start only Webpack server in dev mode:

    cargo xtask ui-dev

Clean build directories:

    cargo xtask clean

Run rsbt cli in Docker to check interaction with popular torrent clients:

    docker-compose up

## Configuration

### Web version

Configuration stored following locations:

| Location                  | Description           |
|---------------------------|-----------------------|
| $HOME/.rsbt/torrents.toml | Current torrents      |
| $HOME/.rsbt/download/     | Default download path |

### CLI version

We use [confy](https://docs.rs/confy) for configuration. Configuration is stored in `toml` format.
