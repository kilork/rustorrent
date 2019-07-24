# Development information

## Schemes

### Torrent states

```graphviz
digraph Peer {
    rankdir=LR
    Active -> Idle [label = "event 'all counters achieved'"]
    Active -> Idle [label = "action 'pause'"]
    Idle -> Active [label = "action 'resume'"]
}
```

### Peer states

```graphviz
digraph Peer {
    rankdir=LR
    size="8,5"
    Connected -> Disconnected -> Connected
}
```

## Processes

### Downloader

This process is a loop to react on download events, select new blocks for download and update counters.

### Peer

Each peer is running its own processing stream.
