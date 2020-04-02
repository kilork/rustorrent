import { Component, OnInit } from '@angular/core';
import { HttpResponse } from '@angular/common/http';
// eslint-disable-next-line @typescript-eslint/no-unused-vars
import { FormBuilder } from '@angular/forms';
import { ActivatedRoute } from '@angular/router';
import { Observable } from 'rxjs';

import { ITorrent, Torrent } from 'app/shared/model/torrent.model';
import { TorrentService } from './torrent.service';
import { JhiLanguageService } from 'ng-jhipster';

@Component({
  selector: 'rt-torrent-update',
  templateUrl: './torrent-update.component.html'
})
export class TorrentUpdateComponent implements OnInit {
  isSaving = false;

  fileToUpload: File | null = null;
  showError = false;

  editForm = this.fb.group({
    id: [],
    name: [],
    file: []
  });

  constructor(
    protected torrentService: TorrentService,
    public languageService: JhiLanguageService,
    protected activatedRoute: ActivatedRoute,
    private fb: FormBuilder
  ) {}

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
      this.subscribeToSaveResponse(this.torrentService.upload(this.fileToUpload!));
    }
  }

  updateFile(files: FileList): void {
    this.fileToUpload = files.item(0);
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
