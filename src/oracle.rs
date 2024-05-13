use cozy_chess::{BitBoard, Board, Color, Piece};

pub struct Oracle {}

impl Oracle {
    pub fn is_draw(board: &Board) -> bool {
        let all_pieces = board.occupied();
        let kings = board.pieces(Piece::King);
        let knights = board.pieces(Piece::Knight);
        let bishops = board.pieces(Piece::Bishop);

        match all_pieces.len() {
            2 => true,                            // K vs K
            3 => !(knights | bishops).is_empty(), // K vs K + N or K vs K + B
            4 => {
                let one_piece_per_colour = board.colors(Color::White).len() == 2;

                // K + N vs K + N and the kings are not on the edges
                if knights.len() == 2 && (kings & BitBoard::EDGES).is_empty() {
                    return true;
                }

                // K + B vs K + B
                if bishops.len() == 2 {
                    // bishops are on the same colour
                    if (bishops & BitBoard::DARK_SQUARES).len() != 1 {
                        return true;
                    }

                    // bishops are on opposite colours and the kings are not in the corners
                    if one_piece_per_colour && (kings & BitBoard::CORNERS).is_empty() {
                        return true;
                    }
                }

                if bishops.len() == 1
                    && knights.len() == 1
                    && one_piece_per_colour
                    && (kings & BitBoard::CORNERS).is_empty()
                {
                    return true;
                }

                false
            }
            _ => false,
        }
    }
}
