import { Injectable } from '@angular/core';
import { HttpResponse } from '@angular/common/http';
import { Resolve, ActivatedRouteSnapshot, Routes, Router } from '@angular/router';
import { JhiResolvePagingParams } from 'ng-jhipster';
import { Observable, of, EMPTY } from 'rxjs';
import { flatMap } from 'rxjs/operators';

import { UserRouteAccessService } from 'app/core/auth/user-route-access-service';
import { ITorrent, Torrent } from 'app/shared/model/torrent.model';
import { TorrentService } from './torrent.service';
import { TorrentComponent } from './torrent.component';
import { TorrentDetailComponent } from './torrent-detail.component';
import { TorrentUpdateComponent } from './torrent-update.component';

@Injectable({ providedIn: 'root' })
export class TorrentResolve implements Resolve<ITorrent> {
  constructor(private service: TorrentService, private router: Router) {}

  resolve(route: ActivatedRouteSnapshot): Observable<ITorrent> | Observable<never> {
    const id = route.params['id'];
    if (id) {
      return this.service.find(id).pipe(
        flatMap((torrent: HttpResponse<Torrent>) => {
          if (torrent.body) {
            return of(torrent.body);
          } else {
            this.router.navigate(['404']);
            return EMPTY;
          }
        })
      );
    }
    return of(new Torrent());
  }
}

export const torrentRoute: Routes = [
  {
    path: '',
    component: TorrentComponent,
    resolve: {
      pagingParams: JhiResolvePagingParams
    },
    data: {
      authorities: ['ROLE_USER'],
      defaultSort: 'id,asc',
      pageTitle: 'rsbt.torrent.home.title'
    },
    canActivate: [UserRouteAccessService]
  },
  {
    path: ':id/view',
    component: TorrentDetailComponent,
    resolve: {
      torrent: TorrentResolve
    },
    data: {
      authorities: ['ROLE_USER'],
      pageTitle: 'rsbt.torrent.home.title'
    },
    canActivate: [UserRouteAccessService]
  },
  {
    path: 'new',
    component: TorrentUpdateComponent,
    resolve: {
      torrent: TorrentResolve
    },
    data: {
      authorities: ['ROLE_USER'],
      pageTitle: 'rsbt.torrent.home.title'
    },
    canActivate: [UserRouteAccessService]
  },
  {
    path: ':id/edit',
    component: TorrentUpdateComponent,
    resolve: {
      torrent: TorrentResolve
    },
    data: {
      authorities: ['ROLE_USER'],
      pageTitle: 'rsbt.torrent.home.title'
    },
    canActivate: [UserRouteAccessService]
  }
];
