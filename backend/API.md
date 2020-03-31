# RSBT API

## GET /api/torrent

List all torrents

```bash
curl http://localhost:8080/api/torrent
```

Response:

```json
[{"id":1,"name":"ferris.gif","received":0,"uploaded":0,"length":349133,"active":true}]
```

## POST /api/torrent/{id}/action

Torrent actions

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
