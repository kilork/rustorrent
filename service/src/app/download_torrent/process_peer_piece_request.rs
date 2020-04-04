use super::*;

pub(crate) async fn process_peer_piece_request(
    peer_states: &mut HashMap<Uuid, PeerState>,
    peer_id: Uuid,
    index: u32,
    begin: u32,
    length: u32,
    storage: &mut TorrentStorage,
) -> Result<(), RsbtError> {
    if let Some(TorrentPeerState::Connected {
        ref mut sender,
        ref mut uploaded,
        ..
    }) = peer_states.get_mut(&peer_id).map(|x| &mut x.state)
    {
        if let Some(piece) = storage.load(index as usize).await? {
            *uploaded += length as usize;
            let block = piece.as_ref()[begin as usize..(begin as usize + length as usize)].to_vec();
            sender
                .send(PeerMessage::Piece {
                    index,
                    begin,
                    block,
                })
                .await?;
        }
    }
    Ok(())
}
