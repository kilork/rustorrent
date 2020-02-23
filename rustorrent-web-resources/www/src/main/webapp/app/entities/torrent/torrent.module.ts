import { NgModule } from '@angular/core';
import { RouterModule } from '@angular/router';

import { RustorrentSharedModule } from 'app/shared/shared.module';
import { TorrentComponent } from './torrent.component';
import { TorrentDetailComponent } from './torrent-detail.component';
import { TorrentUpdateComponent } from './torrent-update.component';
import { TorrentDeleteDialogComponent } from './torrent-delete-dialog.component';
import { torrentRoute } from './torrent.route';

@NgModule({
  imports: [RustorrentSharedModule, RouterModule.forChild(torrentRoute)],
  declarations: [TorrentComponent, TorrentDetailComponent, TorrentUpdateComponent, TorrentDeleteDialogComponent],
  entryComponents: [TorrentDeleteDialogComponent]
})
export class RustorrentTorrentModule {}
