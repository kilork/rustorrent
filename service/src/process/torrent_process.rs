use crate::{
    event::TorrentEvent,
    file_download::FileDownloadStream,
    process::{TorrentProcessHeader, TorrentProcessStatus, TorrentToken},
    request_response::RequestResponse,
    storage::TorrentStorageState,
    types::{
        public::{AnnounceView, FileView, PeerView, TorrentDownloadState},
        Properties,
    },
    RsbtError,
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
    pub(crate) async fn request<T, F, R>(&self, data: T, cmd: F) -> Result<R, RsbtError>
    where
        F: FnOnce(RequestResponse<T, Result<R, RsbtError>>) -> TorrentEvent,
    {
        let (request_response, response) = RequestResponse::new(data);
        self.process
            .broker_sender
            .clone()
            .send(cmd(request_response))
            .await?;
        response.await?
    }

    pub(crate) async fn enable(&mut self) -> Result<(), RsbtError> {
        debug!("enable {}", self.id);

        self.request((), TorrentEvent::Enable).await?;

        self.update_state(TorrentProcessStatus::Enabled).await
    }

    pub(crate) async fn disable(&mut self) -> Result<(), RsbtError> {
        debug!("disable {}", self.id);

        self.request((), TorrentEvent::Disable).await?;

        self.update_state(TorrentProcessStatus::Disabled).await
    }

    async fn update_state(&mut self, state: TorrentProcessStatus) -> Result<(), RsbtError> {
        self.header.state = state;

        Ok(())
    }

    pub(crate) async fn delete(&mut self, files: bool) -> Result<(), RsbtError> {
        debug!("delete {}", self.id);

        self.request(files, TorrentEvent::Delete).await
    }

    pub(crate) async fn peers(&self) -> Result<Vec<PeerView>, RsbtError> {
        debug!("peers for {}", self.id);

        self.request((), TorrentEvent::PeersView).await
    }

    pub(crate) async fn announces(&self) -> Result<Vec<AnnounceView>, RsbtError> {
        debug!("peers for {}", self.id);

        self.request((), TorrentEvent::AnnounceView).await
    }

    pub(crate) async fn files(&self) -> Result<Vec<FileView>, RsbtError> {
        debug!("files for {}", self.id);

        self.request((), TorrentEvent::FilesView).await
    }

    pub(crate) async fn download_file(
        &self,
        file_id: usize,
        range: Option<Range<usize>>,
    ) -> Result<FileDownloadStream, RsbtError> {
        debug!("download file {}", self.id);
        self.request((file_id, range), TorrentEvent::FileDownload)
            .await
    }
}
