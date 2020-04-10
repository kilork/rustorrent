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

class RequestException {
    constructor(response) {
        this.response = response;
    }
}

class UnauthorizedException {
    constructor(response) {
        this.response = response;
    }
}

async function request(url, init_params) {
    let params = init_params || {};
    params.credentials = 'same-origin';
    const response = await fetch(url, params);
    if (!response.ok) {
        if (response.status === 401) {
            throw new UnauthorizedException(response);
        } else {
            throw new RequestException(response);
        }
    }
    const contentType = response.headers.get("content-type");
    if (contentType && contentType.indexOf("application/json") !== -1) {
        return await response.json();
    } else if (contentType && contentType.indexOf("text") !== -1) {
        return await response.text();
    } else {
        return await response.arrayBuffer();
    }
}

async function get(url) {
    return await method_request(url, "GET");
}

async function method_request(url, method, body) {
    let params = {
        method,
    };
    if (body !== undefined) {
        params.body = JSON.stringify(body);
        params.headers = {
            'Content-Type': 'application/json'
        };
    }
    return await request(url, params);
}

async function post(url, body) {
    return await method_request(url, "POST", body);
}

async function put(url, body) {
    return await method_request(url, "PUT", body);
}

async function delete_(url) {
    return await method_request(url, "DELETE");
}


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
        this.loadingElement = document.getElementById('loading');
        this.allElement = document.getElementById('all');
        this.errorsElement = document.getElementById('errors');
        this.authorizedElement = document.getElementById('authorized');
        this.unauthorizedElement = document.getElementById('unauthorized');

        this.stream = new EventSource('/api/stream');
        this.stream.onmessage = async (event) => {
            if (event.data === "connected") {
                return;
            }
            await this.processEvent(JSON.parse(event.data));
        };

    }

    async processEvent(event) {
        let state = event.stat || event.storage;
        await this.updateTorrent(state.id, state);
    }

    async updateTorrent(id, new_state) {
        let torrent = this.torrents.find(t => t.data.id === id);
        console.log(torrent);
        if (torrent) {
            if (new_state.rx !== undefined) {
                torrent.data.rx = new_state.rx;
            }
            if (new_state.tx !== undefined) {
                torrent.data.tx = new_state.tx;
            }
            if (new_state.read !== undefined) {
                torrent.data.read = new_state.read;
            }
            if (new_state.write !== undefined) {
                torrent.data.write = new_state.write;
            }
            if (new_state.left !== undefined) {
                torrent.data.pieces_left = new_state.left;
            }
            torrent.element.innerHTML = this.torrentRow(torrent.data);
        }
    }

    async doModalDeleteSubmit() {
        try {
            let id = this.modalDeleteSubmit.dataset.id;
            let files = this.modalDeleteFiles.checked;

            await delete_(`/api/torrent/${id}?files=${files}`);

            await this.refresh();
            Modal.hideModal();
        } catch (e) {
            Modal.hideModal();
            await this.handleException(e);
        }
    }

    async handleException(e) {
        console.log(e);
        if (e instanceof UnauthorizedException) {
            this.hideAuthorized();
            this.showUnauthorized();
            throw e;
        } else if (e instanceof RequestException) {
            alert(e.response.error);
        } else {
            alert(e);
        }
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
        try {
            let id = input.dataset.id;

            await post(`/api/torrent/${id}/action`, {
                "action": action
            });

            const torrent = await get(`/api/torrent/${id}`);

            if (torrent.error) {
                throw torrent.error;
            }

            let updatedTorrent = this.torrents.find(t => `${t.data.id}` === id);

            if (updatedTorrent) {
                updatedTorrent.data = torrent;
                updatedTorrent.element.innerHTML = this.torrentRow(torrent);
            }
        } catch (e) {
            await this.handleException(e);
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
        try {
            const torrents = await get("/api/torrent");

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
        } catch (e) {
            this.handleException(e);
        }
    }

    hideElement(el) {
        el.classList.add('hide');
    }
    showElement(el) {
        el.classList.remove('hide');
    }

    hideAll() {
        this.hideElement(this.allElement);
    }
    showAll() {
        this.showElement(this.allElement);
    }

    hideLoading() {
        this.hideElement(this.loadingElement);
    }
    showLoading() {
        this.showElement(this.loadingElement);
    }

    hideUnauthorized() {
        this.hideElement(this.unauthorizedElement);
    }
    showUnauthorized() {
        this.showElement(this.unauthorizedElement);
    }

    hideAuthorized() {
        this.hideElement(this.authorizedElement);
    }
    showAuthorized() {
        this.showElement(this.authorizedElement);
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