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
    }

    async refresh() {
        const torrents = await (await fetch("/api/torrent")).json();

        if (torrents.error) {
            throw torrents.error;
        }

        this.torrentsTable.innerHTML = '';

        for (const torrent of torrents) {
            const newElement = document.createElement("tr");
            newElement.innerHTML = `<td>${torrent.id}</td><td>${torrent.name}</td><td>${torrent.length}</td><td>tx: ${torrent.tx}<br>rx: ${torrent.rx}<br>${torrent.pieces_left}/${torrent.pieces_total}</td><td></td>`;
            this.torrentsTable.appendChild(newElement);
        }

    }
}