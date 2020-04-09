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


    torrentActionActive(torrent) {
        if (torrent.active) {
            return `<input type="button" class="button-primary torrent-action torrent-action-disable" data-id="${torrent.id}" title="Pause" value="❚❚" onclick="torrentService.disable(this)">`;
        } else {
            return `<input type="button" class="button-primary torrent-action torrent-action-enable" data-id="${torrent.id}" title="Start" value="▶︎" onclick="torrentService.enable(this)">`;
        }
    }

    torrentRow(torrent) {
        return [
            `${torrent.id}`,
            `<strong>${torrent.name}</strong><br><span class="tip size">${torrent.length}</span>`,
            [
                `<div class="tip upload">${torrent.tx}</div>`,
                `<div class="tip download">${torrent.rx}</div>`,
                `<div class="tip ratio">${torrent.pieces_total - torrent.pieces_left}<span class="ratio-split"></span>${torrent.pieces_total}</div>`
            ].join(''),
            '<div class="torrent-actions">' + [
                this.torrentActionActive(torrent)
            ].join('') + '</div>'
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