import { Injectable } from '@angular/core';
import { HttpClient, HttpResponse } from '@angular/common/http';
import { Observable } from 'rxjs';

import { SERVER_API_URL } from 'app/app.constants';
import { createRequestOption } from 'app/shared/util/request-util';
import { ITorrent, Torrent } from 'app/shared/model/torrent.model';

type EntityResponseType = HttpResponse<ITorrent>;
type EntityArrayResponseType = HttpResponse<ITorrent[]>;

@Injectable({ providedIn: 'root' })
export class TorrentService {
  public resourceUrl = SERVER_API_URL + 'api/torrent';
  public uploadUrl = SERVER_API_URL + 'api/upload';

  constructor(protected http: HttpClient) {}

  create(torrent: ITorrent): Observable<EntityResponseType> {
    return this.http.post<ITorrent>(this.resourceUrl, torrent, { observe: 'response' });
  }

  update(torrent: ITorrent): Observable<EntityResponseType> {
    return this.http.put<ITorrent>(this.resourceUrl, torrent, { observe: 'response' });
  }

  find(id: number): Observable<EntityResponseType> {
    return this.http.get<ITorrent>(`${this.resourceUrl}/${id}`, { observe: 'response' });
  }

  query(req?: any): Observable<EntityArrayResponseType> {
    const options = createRequestOption(req);
    return this.http.get<ITorrent[]>(this.resourceUrl, { params: options, observe: 'response' });
  }

  delete(id: string): Observable<HttpResponse<{}>> {
    return this.http.delete(`${this.resourceUrl}/${id}`, { observe: 'response' });
  }

  upload(torrent: File): Observable<EntityResponseType> {
    const formData: FormData = new FormData();
    formData.append('torrent', torrent, torrent.name);
    return this.http.post(this.uploadUrl, formData, { observe: 'response' });
  }

  executeAction(torrent: Torrent, action: { action: string }): Observable<EntityResponseType> {
    return this.http.post(`${this.resourceUrl}/${torrent.id}/action`, action, { observe: 'response' });
  }
}
