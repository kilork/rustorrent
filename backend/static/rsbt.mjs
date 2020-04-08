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

    torrentRow(torrent) {
        return [
            `${torrent.id}`,
            `<strong>${torrent.name}</strong><br><span class="size" title="Size">${torrent.length}</span>`,
            [
                `<span class="upload"><span class="tx" title="Uploaded">⬆︎</span> ${torrent.tx}</span>`,
                `<span class="download"><span class="rx" title="Downloaded">⬇︎</span> ${torrent.rx}</span>`,
                `${torrent.pieces_left} / ${torrent.pieces_total}`
            ].join('<br>')
        ].map(value => `<td>${value}</td>`).join('')
    }

    async refresh() {
        const torrents = await (await fetch("/api/torrent")).json();

        if (torrents.error) {
            throw torrents.error;
        }

        this.torrentsTable.innerHTML = '';

        for (const torrent of torrents) {
            const newElement = document.createElement("tr");
            newElement.innerHTML = this.torrentRow(torrent);
            this.torrentsTable.appendChild(newElement);
        }

    }
}