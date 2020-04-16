use crate::BLOCK_SIZE;

pub(crate) fn request_message(buffer: &[u8], piece: usize, piece_length: usize) -> (u32, u32, u32) {
    let index = piece as u32;
    let begin = buffer.len() as u32;
    let length = if piece_length - buffer.len() < BLOCK_SIZE {
        piece_length - buffer.len()
    } else {
        BLOCK_SIZE
    } as u32;
    (index, begin, length)
}
