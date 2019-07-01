use super::*;

impl Inner {
    pub(crate) fn command_add_torrent(
        self: Arc<Self>,
        path: PathBuf,
    ) -> Result<Arc<TorrentProcess>, RustorrentError> {
        debug!("Run command: adding torrent from file: {:?}", path);
        let torrent = parse_torrent(&path)?;
        let hash_id = torrent.info_sha1_hash();
        if let Some(process) = self
            .processes
            .read()
            .unwrap()
            .iter()
            .filter(|x| x.hash_id == hash_id)
            .cloned()
            .next()
        {
            warn!("Torrent already in the list: {}", url_encode(&hash_id));
            return Ok(process);
        }
        let info = torrent.info()?;
        let left = info.len();
        let pieces_count = info.pieces.len();
        let pieces = (0..pieces_count)
            .map(|_| Arc::new(Mutex::new(Default::default())))
            .collect();
        let process = Arc::new(TorrentProcess {
            path,
            torrent,
            info,
            hash_id,
            torrent_state: Arc::new(Mutex::new(TorrentProcessState::Init)),
            announce_state: Arc::new(Mutex::new(AnnounceState::Idle)),
            stats: Arc::new(Mutex::new(TorrentProcessStats {
                downloaded: 0,
                uploaded: 0,
                left,
            })),
            blocks_downloading: Arc::new(Mutex::new(HashMap::new())),
            torrent_storage: RwLock::new(TorrentStorage {
                pieces,
                peers: vec![],
            }),
        });
        self.processes.write().unwrap().push(process.clone());
        Ok(process)
    }
}
