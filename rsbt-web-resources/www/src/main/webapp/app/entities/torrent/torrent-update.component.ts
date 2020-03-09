import { Component, OnInit } from '@angular/core';
import { HttpResponse } from '@angular/common/http';
// eslint-disable-next-line @typescript-eslint/no-unused-vars
import { FormBuilder, Validators } from '@angular/forms';
import { ActivatedRoute } from '@angular/router';
import { Observable } from 'rxjs';
import * as moment from 'moment';
import { DATE_TIME_FORMAT } from 'app/shared/constants/input.constants';

import { ITorrent, Torrent } from 'app/shared/model/torrent.model';
import { TorrentService } from './torrent.service';

@Component({
  selector: 'rt-torrent-update',
  templateUrl: './torrent-update.component.html'
})
export class TorrentUpdateComponent implements OnInit {
  isSaving = false;

  editForm = this.fb.group({
    id: [],
    name: []
  });

  constructor(protected torrentService: TorrentService, protected activatedRoute: ActivatedRoute, private fb: FormBuilder) {}

  ngOnInit(): void {
    this.activatedRoute.data.subscribe(({ torrent }) => {
      this.updateForm(torrent);
    });
  }

  updateForm(torrent: ITorrent): void {
    this.editForm.patchValue({
      id: torrent.id,
      name: torrent.name
    });
  }

  previousState(): void {
    window.history.back();
  }

  save(): void {
    this.isSaving = true;
    const torrent = this.createFromForm();
    if (torrent.id !== undefined) {
      this.subscribeToSaveResponse(this.torrentService.update(torrent));
    } else {
      this.subscribeToSaveResponse(this.torrentService.create(torrent));
    }
  }

  private createFromForm(): ITorrent {
    return {
      ...new Torrent(),
      id: this.editForm.get(['id'])!.value,
      name: this.editForm.get(['name'])!.value
    };
  }

  protected subscribeToSaveResponse(result: Observable<HttpResponse<ITorrent>>): void {
    result.subscribe(
      () => this.onSaveSuccess(),
      () => this.onSaveError()
    );
  }

  protected onSaveSuccess(): void {
    this.isSaving = false;
    this.previousState();
  }

  protected onSaveError(): void {
    this.isSaving = false;
  }
}
