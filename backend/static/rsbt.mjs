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
        this.modal = new Modal();
        this.modalDeleteHeader = document.getElementById('modal-delete-header');
        this.modalDeleteFiles = document.getElementById('modal-delete-files');
        this.modalDeleteSubmit = document.getElementById('modal-delete-submit');
        this.modalDeleteSubmit.onclick = (e) => {
            this.doModalDeleteSubmit();
            e.preventDefault();
        };
    }

    async doModalDeleteSubmit() {
        let id = this.modalDeleteSubmit.dataset.id;
        let files = this.modalDeleteFiles.checked;

        await fetch(`/api/torrent/${id}?files=${files}`, {
            method: "DELETE",
        });

        await this.refresh();
        Modal.hideModal();
    }

    delete(input) {
        let id = input.dataset.id;

        this.modalDeleteSubmit.dataset.id = id;

        let deletedTorrent = this.torrents.find(t => `${t.data.id}` === id);

        this.modalDeleteHeader.innerHTML = `Delete ${deletedTorrent.data.name}`;

        this.modal.openModal('modal-delete');
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

    torrentActionInput(action, torrent) {
        return `<input type="button" class="button-primary torrent-action torrent-action-${action.id}" data-id="${torrent.id}" title="${action.title}" value="${action.icon}" onclick="torrentService.${action.id}(this)">`;
    }

    torrentActionActive(torrent) {
        if (torrent.active) {
            return `<input type="button" class="button-primary torrent-action torrent-action-disable" data-id="${torrent.id}" title="Pause" value="❚❚" onclick="torrentService.disable(this)">`;
        } else {
            return `<input type="button" class="button-primary torrent-action torrent-action-enable" data-id="${torrent.id}" title="Start" value="▶︎" onclick="torrentService.enable(this)">`;
        }
    }

    torrentActionDelete(torrent) {
        return this.torrentActionInput({
            id: 'delete',
            title: 'Title',
            icon: 'x'
        }, torrent);
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
                this.torrentActionActive(torrent),
                this.torrentActionDelete(torrent)
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

class Modal {

    constructor() {
        this.close = document.querySelectorAll('.js-close-modal');
        this.modals = document.querySelectorAll('.modal');
        this.modalInners = document.querySelectorAll('.modal-inner');

        this.listeners();
    }

    listeners() {
        window.addEventListener('keydown', this.keyDown);

        this.modals.forEach(el => {
            el.addEventListener('transitionend', this.revealModal, false);
            el.addEventListener('click', this.backdropClose, false);
        });

        this.close.forEach(el => {
            el.addEventListener('click', Modal.hideModal, false);
        });

        this.modalInners.forEach(el => {
            el.addEventListener('transitionend', this.closeModal, false);
        });
    }

    keyDown(e) {
        if (27 === e.keyCode && document.body.classList.contains('modal-body')) {
            Modal.hideModal();
        }
    }

    backdropClose(el) {
        if (!el.target.classList.contains('modal-visible')) {
            return;
        }

        let backdrop = el.currentTarget.dataset.backdrop !== undefined ? el.currentTarget.dataset.backdrop : true;

        if (backdrop === true) {
            Modal.hideModal();
        }
    }

    static hideModal() {
        let modalOpen = document.querySelector('.modal.modal-visible');

        modalOpen.querySelector('.modal-inner').classList.remove('modal-reveal');
        document.querySelector('.modal-body').addEventListener('transitionend', Modal.modalBody, false);
        document.body.classList.add('modal-fadeOut');
    }

    closeModal(el) {
        if ('opacity' === el.propertyName && !el.target.classList.contains('modal-reveal')) {
            document.querySelector('.modal.modal-visible').classList.remove('modal-visible');
        }
    }

    openModal(modalID) {
        let modal = document.getElementById(modalID);

        document.body.classList.add('modal-body');
        modal.classList.add('modal-visible');
    }

    revealModal(el) {
        if ('opacity' === el.propertyName && el.target.classList.contains('modal-visible')) {
            el.target.querySelector('.modal-inner').classList.add('modal-reveal');
        }
    }

    static modalBody(el) {
        if ('opacity' === el.propertyName && el.target.classList.contains('modal') && !el.target.classList.contains('modal-visible')) {
            document.body.classList.remove('modal-body', 'modal-fadeOut');
        }
    }

}