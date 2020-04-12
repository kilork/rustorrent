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
    return await method_request(url, 'POST', body);
}

async function put(url, body) {
    return await method_request(url, 'PUT', body);
}

async function delete_(url) {
    return await method_request(url, 'DELETE');
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

        this.modalFilesCloseElement = document.getElementById('modal-files-close');
        this.modalFilesListElement = document.getElementById('modal-files-list');
        this.modalFilesCloseElement.onclick = (e) => {
            Modal.hideModal();
        };

        this.loadingElement = document.getElementById('loading');
        this.allElement = document.getElementById('all');
        this.authorizedElement = document.getElementById('authorized');
        this.unauthorizedElement = document.getElementById('unauthorized');

        this.stream = new EventSource('/api/stream');
        this.stream.onmessage = async (event) => {
            if (event.data === 'connected') {
                return;
            }
            await this.processEvent(JSON.parse(event.data));
        };

        this.uploadElement = document.getElementById('upload');
        this.torrentElement = document.getElementById('torrent');
        this.uploadElement.onclick = async (e) => {
            await this.upload();
        };

        this.successNotification = window.createNotification({
            theme: 'success'
        });
        this.errorNotification = window.createNotification({
            theme: 'error'
        });
        this.infoNotification = window.createNotification({
            theme: 'info'
        });
        this.warningNotification = window.createNotification({
            theme: 'warning'
        });

    }

    async processEvent(event) {
        let state = event.stat || event.storage;
        await this.updateTorrent(state.id, state);
    }

    async updateTorrent(id, new_state) {
        let torrent = this.torrents.find(t => t.data.id === id);
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
            torrent.stat.innerHTML = this.torrentStats(torrent.data);
        }
    }

    async upload() {
        try {
            let torrent = this.torrentElement.files[0];

            if (torrent) {
                let formData = new FormData();
                formData.append('torrent', torrent);
                let torrentUploaded = await request('/api/upload', {
                    method: 'POST',
                    body: formData
                });
                await this.refresh();
                this.successNotification({
                    message: `Torrent ${torrentUploaded.name}(${torrentUploaded.length}) download started`
                });
                this.torrentElement.value = '';
            } else {
                this.warningNotification({
                    message: 'Please select torrent file to upload'
                });
            }
        } catch (e) {
            await this.handleException(e, () => 'Upload failed');
        }
    }

    async doModalDeleteSubmit() {
        try {
            let id = this.modalDeleteSubmit.dataset.id;
            let files = this.modalDeleteFiles.checked;

            await delete_(`/api/torrent/${id}?files=${files}`);

            await this.refresh();
            Modal.hideModal();
            this.successNotification({
                message: 'Torrent deleted'
            });
        } catch (e) {
            Modal.hideModal();
            await this.handleException(e);
        }
    }

    async handleException(e, context) {
        console.log(e);
        if (e instanceof UnauthorizedException) {
            this.hideAuthorized();
            this.showUnauthorized();
            throw e;
        } else if (e instanceof RequestException) {
            const response = e.response;
            const contentType = response.headers.get("content-type");
            let errorMessage;
            if (contentType && contentType.indexOf("application/json") !== -1) {
                errorMessage = (await response.json()).error;
            } else {
                errorMessage = await response.text();
            }
            let contextMessage = context !== undefined ? context() + ': ' : '';
            this.addError({
                title: `${contextMessage}(${response.status}) ${response.statusText}`,
                message: errorMessage
            });
        } else {
            let contextMessage = context !== undefined ? context() : undefined;
            this.addError({
                title: contextMessage,
                message: `${e.message || e}`
            });
        }
    }

    addError(message) {
        this.errorNotification(message);
    }

    async files(input) {
        try {
            let id = input.dataset.id;

            let fileList = this.modalFilesListElement;

            fileList.innerHTML = '';

            let files = await get(`/api/torrent/${id}/file`)

            for (const file of files) {
                const newElement = document.createElement('tr');
                newElement.innerHTML = this.fileRow(id, file);
                fileList.appendChild(newElement);
            }

            this.modal.openModal('modal-files');
        } catch (e) {
            await this.handleException(e);
        }
    }

    async delete(input) {
        try {
            let id = input.dataset.id;

            this.modalDeleteSubmit.dataset.id = id;

            let deletedTorrent = this.torrents.find(t => `${t.data.id}` === id);

            this.modalDeleteHeader.innerHTML = `Delete ${deletedTorrent.data.name}`;

            this.modal.openModal('modal-delete');
        } catch (e) {
            await this.handleException(e);
        }
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
                updatedTorrent.stat = document.getElementById(`torrent-stat-${id}`)
            }
        } catch (e) {
            await this.handleException(e);
        }
    }

    downloadFile(torrent_id, file_id) {
        window.open(`/api/torrent/${torrent_id}/file/${file_id}/download`, 'Download');
    }

    fileActionInput(action, torrent_id, file) {
        return `<input type="button" class="button-primary file-action file-action-${action.id}" title="${action.title}" value="${action.icon}" onclick="torrentService.${action.method}(${torrent_id}, ${file.id})">`;
    }

    fileActionDownload(torrent_id, file) {
        return this.fileActionInput({
            id: 'download',
            method: 'downloadFile',
            title: 'Download',
            icon: '⬇︎'
        }, torrent_id, file);
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
            title: 'Delete',
            icon: 'x'
        }, torrent);
    }

    torrentActionFiles(torrent) {
        return this.torrentActionInput({
            id: 'files',
            title: 'Files',
            icon: '⬇︎'
        }, torrent);
    }

    torrentStats(torrent) {
        return [
            `<div class="tip upload">${torrent.tx}</div>`,
            `<div class="tip download">${torrent.rx}</div>`,
            `<div class="tip ratio">${torrent.pieces_total - torrent.pieces_left}<span class="ratio-split"></span>${torrent.pieces_total}</div>`
        ].join('');
    }

    torrentRow(torrent) {
        return [
            `${torrent.id}`,
            `<strong>${torrent.name}</strong><br><span class="tip size">${torrent.length}</span>`,
            `<div id="torrent-stat-${torrent.id}">` + this.torrentStats(torrent) + '</div>',
            '<div class="torrent-actions">' + [
                this.torrentActionActive(torrent),
                this.torrentActionFiles(torrent),
                this.torrentActionDelete(torrent)
            ].join('') + '</div>'
        ].map(value => `<td>${value}</td>`).join('')
    }

    fileRow(torrent_id, file) {
        return [
            `${file.id}`,
            `<strong>${file.name}</strong><br>`,
            `<span class="tip size">${file.size}</span>`,
            '<div class="file-actions">' + [
                this.fileActionDownload(torrent_id, file)
            ].join('') + '</div>'
        ].map(value => `<td>${value}</td>`).join('')
    }

    async refresh() {
        try {
            const torrents = await get('/api/torrent');

            if (torrents.error) {
                throw torrents.error;
            }

            this.torrentsTable.innerHTML = '';
            this.torrents = [];

            for (const torrent of torrents) {
                const newElement = document.createElement('tr');
                newElement.innerHTML = this.torrentRow(torrent);
                this.torrentsTable.appendChild(newElement);
                this.torrents.push({
                    element: newElement,
                    data: torrent,
                    stat: document.getElementById(`torrent-stat-${torrent.id}`)
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

/* Notifications */
/* https://github.com/JamieLivingstone/styled-notifications */
! function (t) {
    function n(i) {
        if (e[i]) return e[i].exports;
        var o = e[i] = {
            i: i,
            l: !1,
            exports: {}
        };
        return t[i].call(o.exports, o, o.exports, n), o.l = !0, o.exports
    }
    var e = {};
    n.m = t, n.c = e, n.d = function (t, e, i) {
        n.o(t, e) || Object.defineProperty(t, e, {
            configurable: !1,
            enumerable: !0,
            get: i
        })
    }, n.n = function (t) {
        var e = t && t.__esModule ? function () {
            return t.default
        } : function () {
            return t
        };
        return n.d(e, "a", e), e
    }, n.o = function (t, n) {
        return Object.prototype.hasOwnProperty.call(t, n)
    }, n.p = "", n(n.s = 0)
}([function (t, n, e) {
    e(1), t.exports = e(4)
}, function (t, n, e) {
    "use strict";
    var i = Object.assign || function (t) {
        for (var n = 1; n < arguments.length; n++) {
            var e = arguments[n];
            for (var i in e) Object.prototype.hasOwnProperty.call(e, i) && (t[i] = e[i])
        }
        return t
    };
    e(2);
    var o = e(3);
    ! function (t) {
        function n(t) {
            return t = i({}, c, t),
                function (t) {
                    return ["nfc-top-left", "nfc-top-right", "nfc-bottom-left", "nfc-bottom-right"].indexOf(t) > -1
                }(t.positionClass) || (console.warn("An invalid notification position class has been specified."), t.positionClass = c.positionClass), t.onclick && "function" != typeof t.onclick && (console.warn("Notification on click must be a function."), t.onclick = c.onclick), "number" != typeof t.showDuration && (t.showDuration = c.showDuration), (0, o.isString)(t.theme) && 0 !== t.theme.length || (console.warn("Notification theme must be a string with length"), t.theme = c.theme), t
        }

        function e(t) {
            return t = n(t),
                function () {
                    var n = arguments.length > 0 && void 0 !== arguments[0] ? arguments[0] : {},
                        e = n.title,
                        i = n.message,
                        c = r(t.positionClass);
                    if (!e && !i) return console.warn("Notification must contain a title or a message!");
                    var a = (0, o.createElement)("div", "ncf", t.theme);
                    if (!0 === t.closeOnClick && a.addEventListener("click", function () {
                            return c.removeChild(a)
                        }), t.onclick && a.addEventListener("click", function (n) {
                            return t.onclick(n)
                        }), t.displayCloseButton) {
                        var s = (0, o.createElement)("button");
                        s.innerText = "X", !1 === t.closeOnClick && s.addEventListener("click", function () {
                            return c.removeChild(a)
                        }), (0, o.append)(a, s)
                    }
                    if ((0, o.isString)(e) && e.length && (0, o.append)(a, (0, o.createParagraph)("ncf-title")(e)), (0, o.isString)(i) && i.length && (0, o.append)(a, (0, o.createParagraph)("nfc-message")(i)), (0, o.append)(c, a), t.showDuration && t.showDuration > 0) {
                        var l = setTimeout(function () {
                            c.removeChild(a), 0 === c.querySelectorAll(".ncf").length && document.body.removeChild(c)
                        }, t.showDuration);
                        (t.closeOnClick || t.displayCloseButton) && a.addEventListener("click", function () {
                            return clearTimeout(l)
                        })
                    }
                }
        }

        function r(t) {
            var n = document.querySelector("." + t);
            return n || (n = (0, o.createElement)("div", "ncf-container", t), (0, o.append)(document.body, n)), n
        }
        var c = {
            closeOnClick: !0,
            displayCloseButton: !1,
            positionClass: "nfc-top-right",
            onclick: !1,
            showDuration: 3500,
            theme: "success"
        };
        t.createNotification ? console.warn("Window already contains a create notification function. Have you included the script twice?") : t.createNotification = e
    }(window)
}, function (t, n, e) {
    "use strict";
    ! function () {
        function t(t) {
            this.el = t;
            for (var n = t.className.replace(/^\s+|\s+$/g, "").split(/\s+/), i = 0; i < n.length; i++) e.call(this, n[i])
        }
        if (!(void 0 === window.Element || "classList" in document.documentElement)) {
            var n = Array.prototype,
                e = n.push,
                i = n.splice,
                o = n.join;
            t.prototype = {
                    add: function (t) {
                        this.contains(t) || (e.call(this, t), this.el.className = this.toString())
                    },
                    contains: function (t) {
                        return -1 != this.el.className.indexOf(t)
                    },
                    item: function (t) {
                        return this[t] || null
                    },
                    remove: function (t) {
                        if (this.contains(t)) {
                            for (var n = 0; n < this.length && this[n] != t; n++);
                            i.call(this, n, 1), this.el.className = this.toString()
                        }
                    },
                    toString: function () {
                        return o.call(this, " ")
                    },
                    toggle: function (t) {
                        return this.contains(t) ? this.remove(t) : this.add(t), this.contains(t)
                    }
                }, window.DOMTokenList = t,
                function (t, n, e) {
                    Object.defineProperty ? Object.defineProperty(t, n, {
                        get: e
                    }) : t.__defineGetter__(n, e)
                }(Element.prototype, "classList", function () {
                    return new t(this)
                })
        }
    }()
}, function (t, n, e) {
    "use strict";
    Object.defineProperty(n, "__esModule", {
        value: !0
    });
    var i = n.partial = function (t) {
            for (var n = arguments.length, e = Array(n > 1 ? n - 1 : 0), i = 1; i < n; i++) e[i - 1] = arguments[i];
            return function () {
                for (var n = arguments.length, i = Array(n), o = 0; o < n; o++) i[o] = arguments[o];
                return t.apply(void 0, e.concat(i))
            }
        },
        o = (n.append = function (t) {
            for (var n = arguments.length, e = Array(n > 1 ? n - 1 : 0), i = 1; i < n; i++) e[i - 1] = arguments[i];
            return e.forEach(function (n) {
                return t.appendChild(n)
            })
        }, n.isString = function (t) {
            return "string" == typeof t
        }, n.createElement = function (t) {
            for (var n = arguments.length, e = Array(n > 1 ? n - 1 : 0), i = 1; i < n; i++) e[i - 1] = arguments[i];
            var o = document.createElement(t);
            return e.length && e.forEach(function (t) {
                return o.classList.add(t)
            }), o
        }),
        r = function (t, n) {
            return t.innerText = n, t
        },
        c = function (t) {
            for (var n = arguments.length, e = Array(n > 1 ? n - 1 : 0), c = 1; c < n; c++) e[c - 1] = arguments[c];
            return i(r, o.apply(void 0, [t].concat(e)))
        };
    n.createParagraph = function () {
        for (var t = arguments.length, n = Array(t), e = 0; e < t; e++) n[e] = arguments[e];
        return c.apply(void 0, ["p"].concat(n))
    }
}, function (t, n) {}]);
/* Notifications end */