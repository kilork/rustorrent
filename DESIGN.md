# Development information

## Technical specification

RSBT uses BEPs: <http://bittorrent.org/beps/bep_0000.html>

## Processes

### Downloader

This process is a loop to react on download events, select new blocks for download and update counters.

### Peer

Each peer is running its own processing stream.
