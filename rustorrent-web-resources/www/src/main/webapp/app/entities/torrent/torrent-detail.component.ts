import { Component, OnInit } from '@angular/core';
import { ActivatedRoute } from '@angular/router';

import { ITorrent } from 'app/shared/model/torrent.model';

@Component({
  selector: 'rt-torrent-detail',
  templateUrl: './torrent-detail.component.html'
})
export class TorrentDetailComponent implements OnInit {
  torrent: ITorrent | null = null;

  constructor(protected activatedRoute: ActivatedRoute) {}

  ngOnInit(): void {
    this.activatedRoute.data.subscribe(({ torrent }) => (this.torrent = torrent));
  }

  previousState(): void {
    window.history.back();
  }
}
