# RSBT API

## Common aggrements

If exception occurs - output is HTTP message with JSON {"error":"error message"} body.

```json
{"error":"torrent with id 3 not found"}
```

## GET /api/torrent

List all torrents

```bash
curl http://localhost:8080/api/torrent
```

Response:

```json
[{"id":1,"name":"big-buck-bunny","write":5242880,"read":0,"tx":0,"rx":5652480,"pieces_total":1055,"pieces_left":1035,"piece_size":262144,"length":276445467,"active":true},{"id":2,"name":"ferris.gif","write":0,"read":0,"tx":0,"rx":0,"pieces_total":2,"pieces_left":2,"piece_size":262144,"length":349133,"active":true}]
```

Attributes:

- `id` : torrent's id.
- `name` : torrent's name.
- `write` : total bytes written to disk.
- `read` : total bytes read from disk.
- `tx` : total network bytes sended for the torrent.
- `rx` : total network bytes received for the torrent.
- `pieces_total` : total pieces count (torrent consists of same size blocks called pieces).
- `pieces_left` : count of pieces left to download.
- `piece_size` : a size of single piece in bytes.
- `length` : total size of torrent files in bytes.
- `active` : is torrent enabled (true) or disabled (false).

## DELETE /api/torrent/{id}[?files=true|false]

Delete torrent. Optional parameter `files` allows to delete also downloaded torrent data.

## GET /api/stream

Server-Sent Event stream with state updates. Each message comes as json:

```json
...
{"stat":{"id":1,"rx":65077248,"tx":0}}
{"stat":{"id":1,"rx":65241088,"tx":0}}
{"stat":{"id":1,"rx":65355776,"tx":0}}
{"storage":{"id":1,"write":65011712,"read":0,"left":807}}
{"stat":{"id":1,"rx":65617920,"tx":0}}
{"storage":{"id":1,"write":65273856,"read":0,"left":806}}
{"stat":{"id":1,"rx":65880064,"tx":0}}
{"storage":{"id":1,"write":65798144,"read":0,"left":804}}
...
```

`stat` message shows current upload (`tx`) / download (`rx`) statistics for torrent with `id`. This includes all downloaded traffic.

`storage` message shows, how much data was actually readed from disk (`read`), or saved to disk (`write`). `left` is the count of pieces left to download.

Messages in stream for each torrent produced with minimal 0.5 seconds delay to not overload UI.

## POST /api/torrent/{id}/action

Torrent actions.

### Enable torrent

```bash
curl -v \
  --header "Content-Type: application/json" \
  --data '{"action":"enable"}' \
  http://localhost:8080/api/torrent/1/action
```

### Disable torrent

```bash
curl -v \
  --header "Content-Type: application/json" \
  --data '{"action":"disable"}' \
  http://localhost:8080/api/torrent/1/action
```

## GET /api/torrent/{id}/peer

Torrent peers.

```bash
curl http://localhost:8080/api/torrent/2/peer
```

```json
[
{"addr":"78.46.190.97:50007","state":{"connected":{"chocked":true,"interested":false,"rx":0,"tx":0}}},
{"addr":"116.86.22.69:6881","state":{"connected":{"chocked":false,"interested":false,"rx":0,"tx":0}}},
{"addr":"84.229.184.198:51413","state":{"connecting":{}}},
{"addr":"185.192.69.98:41283","state":{"idle":{}}},
/* ... */
]
```

`addr` is the socket address of peer. `state` field is the map with single member, which represents one of possible peer states.

### `idle` state

Not visible from API perspective usually.

### `connecting` state

Client tries to establish connection with this peer.

### `connected` state

Client connected to peer.

Attributes:

- `chocked` : other side disallows to request pieces.
- `interested` : other side wants to request pieces.
- `rx` : bytes downloaded from peer.
- `tx` : bytes uploaded to peer.
- `piece` : currently requested piece.

## GET /api/torrent/{id}/announce

Torrent announce urls.

```bash
curl http://localhost:8080/api/torrent/2/announce
```

```json
[
{"url":"udp://tracker.leechers-paradise.org:6969"}
]
```
