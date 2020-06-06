use crate::{
    event::TorrentEvent,
    file_download::FileDownloadStream,
    process::{TorrentProcessHeader, TorrentProcessStatus, TorrentToken},
    request_response::RequestResponse,
    result::RsbtResult,
    storage::TorrentStorageState,
    types::{
        public::{AnnounceView, FileView, PeerView, TorrentDownloadState},
        Properties,
    },
};
use log::debug;
use std::{ops::Range, sync::Arc};
use tokio::sync::watch;

#[derive(Debug, Clone)]
pub struct TorrentProcess {
    pub id: usize,
    pub name: String,
    pub header: TorrentProcessHeader,
    pub process: Arc<TorrentToken>,
    pub properties: Arc<Properties>,
    pub storage_state_watch: watch::Receiver<TorrentStorageState>,
    pub statistics_watch: watch::Receiver<TorrentDownloadState>,
}

impl TorrentProcess {
    pub(crate) async fn request<T, F, R>(&self, data: T, cmd: F) -> RsbtResult<R>
    where
        F: FnOnce(RequestResponse<T, RsbtResult<R>>) -> TorrentEvent,
    {
        let (request_response, response) = RequestResponse::new(data);
        self.process
            .broker_sender
            .clone()
            .send(cmd(request_response))
            .await?;
        response.await?
    }

    pub(crate) async fn enable(&mut self) -> RsbtResult<()> {
        debug!("enable {}", self.id);

        self.request((), TorrentEvent::Enable).await?;

        self.update_state(TorrentProcessStatus::Enabled).await
    }

    pub(crate) async fn disable(&mut self) -> RsbtResult<()> {
        debug!("disable {}", self.id);

        self.request((), TorrentEvent::Disable).await?;

        self.update_state(TorrentProcessStatus::Disabled).await
    }

    async fn update_state(&mut self, state: TorrentProcessStatus) -> RsbtResult<()> {
        self.header.state = state;

        Ok(())
    }

    pub(crate) async fn delete(&mut self, files: bool) -> RsbtResult<()> {
        debug!("delete {}", self.id);

        self.request(files, TorrentEvent::Delete).await
    }

    pub(crate) async fn peers(&self) -> RsbtResult<Vec<PeerView>> {
        debug!("peers for {}", self.id);

        self.request((), TorrentEvent::PeersView).await
    }

    pub(crate) async fn announces(&self) -> RsbtResult<Vec<AnnounceView>> {
        debug!("peers for {}", self.id);

        self.request((), TorrentEvent::AnnounceView).await
    }

    pub(crate) async fn files(&self) -> RsbtResult<Vec<FileView>> {
        debug!("files for {}", self.id);

        self.request((), TorrentEvent::FilesView).await
    }

    pub(crate) async fn download_file(
        &self,
        file_id: usize,
        range: Option<Range<usize>>,
    ) -> RsbtResult<FileDownloadStream> {
        debug!("download file {}", self.id);
        self.request((file_id, range), TorrentEvent::FileDownload)
            .await
    }
}
