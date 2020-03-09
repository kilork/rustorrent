import { Moment } from 'moment';

export interface ITorrent {
  id?: string;
  name?: string;
}

export class Torrent implements ITorrent {
  constructor(public id?: string, public name?: string) {}
}
