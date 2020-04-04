# Development information

## Technical specification

RSBT uses BitTorrent Enhancement Proposals (BEPs): <http://bittorrent.org/beps/bep_0000.html>

### Currently implemented BEPs

| BEP                                                   | Description                           |
|-------------------------------------------------------|---------------------------------------|
| [0003](https://www.bittorrent.org/beps/bep_0003.html) | The BitTorrent Protocol Specification |
| [0015](https://www.bittorrent.org/beps/bep_0015.html) | UDP Tracker Protocol for BitTorrent   |
| [0023](https://www.bittorrent.org/beps/bep_0023.html) | Tracker Returns Compact Peer Lists    |

### Pending implementation BEPs

| BEP                                                   | Description                                 |
|-------------------------------------------------------|---------------------------------------------|
| [0005](https://www.bittorrent.org/beps/bep_0005.html) | DHT Protocol                                |
| [0006](https://www.bittorrent.org/beps/bep_0006.html) | Fast Extension                              |
| [0009](https://www.bittorrent.org/beps/bep_0009.html) | Extension for Peers to Send Metadata Files  |
| [0010](https://www.bittorrent.org/beps/bep_0010.html) | Extension Protocol                          |
| [0011](https://www.bittorrent.org/beps/bep_0011.html) | Peer Exchange (PEX)                         |
| [0012](https://www.bittorrent.org/beps/bep_0012.html) | Multitracker Metadata Extension             |
| [0014](https://www.bittorrent.org/beps/bep_0014.html) | Local Service Discovery                     |
| [0019](https://www.bittorrent.org/beps/bep_0019.html) | WebSeed - HTTP/FTP Seeding (GetRight style) |
| [0027](https://www.bittorrent.org/beps/bep_0027.html) | Private Torrents                            |
| [0029](https://www.bittorrent.org/beps/bep_0029.html) | uTorrent transport protocol                 |
| [0055](https://www.bittorrent.org/beps/bep_0055.html) | Holepunch extension                         |

## Processes

### Downloader

This process is a loop to react on download events, select new blocks for download and update counters.

### Peer

Each peer is running its own processing stream.
