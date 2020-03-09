import { Component } from '@angular/core';
import { NgbActiveModal } from '@ng-bootstrap/ng-bootstrap';
import { JhiEventManager } from 'ng-jhipster';

import { ITorrent } from 'app/shared/model/torrent.model';
import { TorrentService } from './torrent.service';

@Component({
  templateUrl: './torrent-delete-dialog.component.html'
})
export class TorrentDeleteDialogComponent {
  torrent?: ITorrent;

  constructor(protected torrentService: TorrentService, public activeModal: NgbActiveModal, protected eventManager: JhiEventManager) {}

  cancel(): void {
    this.activeModal.dismiss();
  }

  confirmDelete(id: string): void {
    this.torrentService.delete(id).subscribe(() => {
      this.eventManager.broadcast('torrentListModification');
      this.activeModal.close();
    });
  }
}
