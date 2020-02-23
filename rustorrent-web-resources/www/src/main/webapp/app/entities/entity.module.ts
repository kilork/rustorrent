import { NgModule } from '@angular/core';
import { RouterModule } from '@angular/router';

@NgModule({
  imports: [
    RouterModule.forChild([
      {
        path: 'torrent',
        loadChildren: () => import('./torrent/torrent.module').then(m => m.RustorrentTorrentModule)
      }
      /* jhipster-needle-add-entity-route - JHipster will add entity modules routes here */
    ])
  ]
})
export class RustorrentEntityModule {}
