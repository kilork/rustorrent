#[derive(Debug, PartialEq)]
pub(crate) struct UdpTrackerScrape {
    pub(crate) complete: i32,
    pub(crate) downloaded: i32,
    pub(crate) incomplete: i32,
}
