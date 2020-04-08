export {
    timeout,
    TorrentService
};

function timeout(interval) {
    return new Promise((resolve, reject) => {
        setTimeout(function () {
            resolve(true);
        }, interval);
    });
};

class TorrentService {
    constructor(torrentsTable) {
        this.torrentsTable = torrentsTable;
        this.torrents = [];
    }

    async enable(input) {
        await this.action(input, "enable");
    }

    async disable(input) {
        await this.action(input, "disable");
    }

    async action(input, action) {
        let id = input.dataset.id;

        let response = await fetch(`/api/torrent/${id}/action`, {
            method: "POST",
            body: JSON.stringify({
                "action": action
            }),
            headers: {
                'Content-Type': 'application/json'
            }
        });

        const torrent = await (await fetch(`/api/torrent/${id}`)).json();

        if (torrent.error) {
            throw torrent.error;
        }

        let updatedTorrent = this.torrents.find(t => `${t.data.id}` === id);

        if (updatedTorrent) {
            updatedTorrent.torrent = torrent;
            updatedTorrent.element.innerHTML = this.torrentRow(torrent);
        }
    }


    torrentRow(torrent) {
        return [
            `${torrent.id}`,
            `<strong>${torrent.name}</strong><br><span class="size" title="Size">${torrent.length}</span>`,
            [
                `<div class="upload">${torrent.tx}</div>`,
                `<div class="download">${torrent.rx}</div>`,
                `<div class="ratio">${torrent.pieces_left}<span class="ratio-split"></span>${torrent.pieces_total}</div>`
            ].join(''),
            [
                torrent.active ? `<input type="button" class="button-primary torrent-action torrent-action-disable" data-id="${torrent.id}" title="Pause" value="❚❚" onclick="torrentService.disable(this)">` : `<input type="button" class="button-primary torrent-action torrent-action-enable" data-id="${torrent.id}" title="Start" value="▶︎" onclick="torrentService.enable(this)">`
            ].join('')
        ].map(value => `<td>${value}</td>`).join('')
    }

    async refresh() {
        const torrents = await (await fetch("/api/torrent")).json();

        if (torrents.error) {
            throw torrents.error;
        }

        this.torrentsTable.innerHTML = '';
        this.torrents = [];

        for (const torrent of torrents) {
            const newElement = document.createElement("tr");
            newElement.innerHTML = this.torrentRow(torrent);
            this.torrentsTable.appendChild(newElement);
            this.torrents.push({
                element: newElement,
                data: torrent
            })
        }

    }
}