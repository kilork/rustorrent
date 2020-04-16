/// Adds matching (new) pieces ( downloaded_pieces[i] & a ) to pieces (list of indexes).
pub(crate) fn match_pieces(pieces: &mut Vec<usize>, downloaded_pieces: &[u8], i: usize, a: u8) {
    let new = if let Some(d) = downloaded_pieces.get(i) {
        a & !d
    } else {
        a
    };

    for j in 0..8 {
        if new & (0b1000_0000 >> j) != 0 {
            pieces.push(i * 8 + j);
        }
    }
}
