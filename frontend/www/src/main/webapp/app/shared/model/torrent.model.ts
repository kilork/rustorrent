export interface ITorrent {
  id?: string;
  name?: string;
  active?: boolean;
}

export class Torrent implements ITorrent {
  constructor(public id?: string, public name?: string, public active?: boolean) {}
}
