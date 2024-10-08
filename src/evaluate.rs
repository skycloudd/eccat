use cozy_chess::{BitBoard, Board, Color, Piece};

#[must_use]
pub fn evaluate(board: &Board) -> Eval {
    let mut mg = 0;
    let mut eg = 0;
    let mut game_phase = 0;

    for square in board.occupied() {
        if let (Some(piece), Some(piece_colour)) = (board.piece_on(square), board.color_on(square))
        {
            let colour_sign = match piece_colour {
                Color::White => 1,
                Color::Black => -1,
            };

            let (mg_value, endgame_value) = piece_square(piece, piece_colour, square);

            mg += mg_value * colour_sign;
            eg += endgame_value * colour_sign;

            game_phase += match piece {
                Piece::Pawn | Piece::King => 0,
                Piece::Knight | Piece::Bishop => 1,
                Piece::Rook => 2,
                Piece::Queen => 4,
            };

            if piece == Piece::Pawn {
                let pawn_files = pawns_in_front_adjacent_files(square, piece_colour);

                let pawns_in_front = pawn_files & board.colored_pieces(!piece_colour, Piece::Pawn);

                if pawns_in_front.is_empty() {
                    let rank = match piece_colour {
                        Color::White => square.rank(),
                        Color::Black => square.rank().flip(),
                    };

                    mg += MG_PASSED_PAWN_BONUS[rank as usize] * colour_sign;
                    eg += EG_PASSED_PAWN_BONUS[rank as usize] * colour_sign;
                }
            }
        }
    }

    if board.colored_pieces(Color::White, Piece::Bishop).len() >= 2 {
        mg += MG_BISHOP_PAIR_BONUS;
        eg += EG_BISHOP_PAIR_BONUS;
    }

    if board.colored_pieces(Color::Black, Piece::Bishop).len() >= 2 {
        mg -= MG_BISHOP_PAIR_BONUS;
        eg -= EG_BISHOP_PAIR_BONUS;
    }

    for file in cozy_chess::File::ALL {
        let file = file.bitboard();

        let white_pawns = board.colored_pieces(Color::White, Piece::Pawn) & file;
        let black_pawns = board.colored_pieces(Color::Black, Piece::Pawn) & file;

        if white_pawns.len() > 1 {
            mg += MG_DOUBLED_PAWNS_PENALTY;
            eg += EG_DOUBLED_PAWNS_PENALTY;
        }

        if black_pawns.len() > 1 {
            mg -= MG_DOUBLED_PAWNS_PENALTY;
            eg -= EG_DOUBLED_PAWNS_PENALTY;
        }
    }

    let tempo = 1 - 2 * (board.side_to_move() as Eval);

    mg += MG_TEMPO * tempo;
    eg += EG_TEMPO * tempo;

    let mg_game_phase = core::cmp::min(24, game_phase);
    let endgame_game_phase = 24 - mg_game_phase;

    let eval = mg
        .saturating_mul(mg_game_phase)
        .saturating_add(eg.saturating_mul(endgame_game_phase))
        .saturating_div(24);

    match board.side_to_move() {
        Color::White => eval,
        Color::Black => -eval,
    }
}

#[inline]
fn pawns_in_front_adjacent_files(square: cozy_chess::Square, piece_colour: Color) -> BitBoard {
    let file = square.file();
    let pawn_files = file.bitboard() | file.adjacent();

    let rank = square.rank();

    cozy_chess::BitBoard(match piece_colour {
        Color::White => pawn_files.0 << ((rank as usize + 1) * 8),
        Color::Black => pawn_files.0 >> ((8 - rank as usize) * 8),
    })
}

#[inline]
const fn piece_square(
    piece: Piece,
    piece_colour: Color,
    square: cozy_chess::Square,
) -> (Eval, Eval) {
    let square_idx = match piece_colour {
        Color::White => square.flip_rank() as usize,
        Color::Black => square as usize,
    };

    let piece_idx = piece as usize;

    (
        MG_PIECE_SQUARE_TABLES[piece_idx][square_idx],
        EG_PIECE_SQUARE_TABLES[piece_idx][square_idx],
    )
}

const fn gen_piece_square_tables(
    tables: &[[Eval; 64]; 6],
    piece_values: [Eval; 6],
) -> [[Eval; 64]; 6] {
    let mut result = [[0; 64]; 6];

    let mut table_idx = 0;

    while table_idx < 6 {
        let mut square_idx = 0;

        while square_idx < 64 {
            result[table_idx][square_idx] = tables[table_idx][square_idx] + piece_values[table_idx];

            square_idx += 1;
        }

        table_idx += 1;
    }

    result
}

const MG_PIECE_SQUARE_TABLES: [[Eval; 64]; 6] = gen_piece_square_tables(
    &[
        MG_PAWN_TABLE,
        MG_KNIGHT_TABLE,
        MG_BISHOP_TABLE,
        MG_ROOK_TABLE,
        MG_QUEEN_TABLE,
        MG_KING_TABLE,
    ],
    MG_PIECE_VALUES,
);

const EG_PIECE_SQUARE_TABLES: [[Eval; 64]; 6] = gen_piece_square_tables(
    &[
        EG_PAWN_TABLE,
        EG_KNIGHT_TABLE,
        EG_BISHOP_TABLE,
        EG_ROOK_TABLE,
        EG_QUEEN_TABLE,
        EG_KING_TABLE,
    ],
    EG_PIECE_VALUES,
);

const MG_PIECE_VALUES: [Eval; 6] = [82, 337, 365, 477, 1025, 0];
const EG_PIECE_VALUES: [Eval; 6] = [94, 281, 297, 512, 936, 0];

const MG_PASSED_PAWN_BONUS: [Eval; 8] = [0, 0, 5, 10, 15, 20, 30, 0];
const EG_PASSED_PAWN_BONUS: [Eval; 8] = [0, 10, 20, 35, 60, 100, 200, 0];

const MG_BISHOP_PAIR_BONUS: Eval = 50;
const EG_BISHOP_PAIR_BONUS: Eval = 20;

const MG_DOUBLED_PAWNS_PENALTY: Eval = -10;
const EG_DOUBLED_PAWNS_PENALTY: Eval = -10;

const MG_TEMPO: Eval = 20;
const EG_TEMPO: Eval = 5;

#[rustfmt::skip]
const MG_PAWN_TABLE: [Eval; 64] = [
    0,   0,   0,   0,   0,   0,  0,   0,
   98, 134,  61,  95,  68, 126, 34, -11,
   -6,   7,  26,  31,  65,  56, 25, -20,
  -14,  13,   6,  21,  23,  12, 17, -23,
  -27,  -2,  -5,  12,  17,   6, 10, -25,
  -26,  -4,  -4, -10,   3,   3, 33, -12,
  -35,  -1, -20, -23, -15,  24, 38, -22,
    0,   0,   0,   0,   0,   0,  0,   0,
];

#[rustfmt::skip]
const EG_PAWN_TABLE: [Eval; 64] = [
    0,   0,   0,   0,   0,   0,   0,   0,
  178, 173, 158, 134, 147, 132, 165, 187,
   94, 100,  85,  67,  56,  53,  82,  84,
   32,  24,  13,   5,  -2,   4,  17,  17,
   13,   9,  -3,  -7,  -7,  -8,   3,  -1,
    4,   7,  -6,   1,   0,  -5,  -1,  -8,
   13,   8,   8,  10,  13,   0,   2,  -7,
    0,   0,   0,   0,   0,   0,   0,   0,
];

#[rustfmt::skip]
const MG_KNIGHT_TABLE: [Eval; 64] = [
  -167, -89, -34, -49,  61, -97, -15, -107,
   -73, -41,  72,  36,  23,  62,   7,  -17,
   -47,  60,  37,  65,  84, 129,  73,   44,
    -9,  17,  19,  53,  37,  69,  18,   22,
   -13,   4,  16,  13,  28,  19,  21,   -8,
   -23,  -9,  12,  10,  19,  17,  25,  -16,
   -29, -53, -12,  -3,  -1,  18, -14,  -19,
  -105, -21, -58, -33, -17, -28, -19,  -23,
];

#[rustfmt::skip]
const EG_KNIGHT_TABLE: [Eval; 64] = [
  -58, -38, -13, -28, -31, -27, -63, -99,
  -25,  -8, -25,  -2,  -9, -25, -24, -52,
  -24, -20,  10,   9,  -1,  -9, -19, -41,
  -17,   3,  22,  22,  22,  11,   8, -18,
  -18,  -6,  16,  25,  16,  17,   4, -18,
  -23,  -3,  -1,  15,  10,  -3, -20, -22,
  -42, -20, -10,  -5,  -2, -20, -23, -44,
  -29, -51, -23, -15, -22, -18, -50, -64,
];

#[rustfmt::skip]
const MG_BISHOP_TABLE: [Eval; 64] = [
  -29,   4, -82, -37, -25, -42,   7,  -8,
  -26,  16, -18, -13,  30,  59,  18, -47,
  -16,  37,  43,  40,  35,  50,  37,  -2,
   -4,   5,  19,  50,  37,  37,   7,  -2,
   -6,  13,  13,  26,  34,  12,  10,   4,
    0,  15,  15,  15,  14,  27,  18,  10,
    4,  15,  16,   0,   7,  21,  33,   1,
  -33,  -3, -14, -21, -13, -12, -39, -21,
];

#[rustfmt::skip]
const EG_BISHOP_TABLE: [Eval; 64] = [
  -14, -21, -11,  -8, -7,  -9, -17, -24,
   -8,  -4,   7, -12, -3, -13,  -4, -14,
    2,  -8,   0,  -1, -2,   6,   0,   4,
   -3,   9,  12,   9, 14,  10,   3,   2,
   -6,   3,  13,  19,  7,  10,  -3,  -9,
  -12,  -3,   8,  10, 13,   3,  -7, -15,
  -14, -18,  -7,  -1,  4,  -9, -15, -27,
  -23,  -9, -23,  -5, -9, -16,  -5, -17,
];

#[rustfmt::skip]
const MG_ROOK_TABLE: [Eval; 64] = [
   32,  42,  32,  51, 63,  9,  31,  43,
   27,  32,  58,  62, 80, 67,  26,  44,
   -5,  19,  26,  36, 17, 45,  61,  16,
  -24, -11,   7,  26, 24, 35,  -8, -20,
  -36, -26, -12,  -1,  9, -7,   6, -23,
  -45, -25, -16, -17,  3,  0,  -5, -33,
  -44, -16, -20,  -9, -1, 11,  -6, -71,
  -19, -13,   1,  17, 16,  7, -37, -26,
];

#[rustfmt::skip]
const EG_ROOK_TABLE: [Eval; 64] = [
  13, 10, 18, 15, 12,  12,   8,   5,
  11, 13, 13, 11, -3,   3,   8,   3,
   7,  7,  7,  5,  4,  -3,  -5,  -3,
   4,  3, 13,  1,  2,   1,  -1,   2,
   3,  5,  8,  4, -5,  -6,  -8, -11,
  -4,  0, -5, -1, -7, -12,  -8, -16,
  -6, -6,  0,  2, -9,  -9, -11,  -3,
  -9,  2,  3, -1, -5, -13,   4, -20,
];

#[rustfmt::skip]
const MG_QUEEN_TABLE: [Eval; 64] = [
  -28,   0,  29,  12,  59,  44,  43,  45,
  -24, -39,  -5,   1, -16,  57,  28,  54,
  -13, -17,   7,   8,  29,  56,  47,  57,
  -27, -27, -16, -16,  -1,  17,  -2,   1,
   -9, -26,  -9, -10,  -2,  -4,   3,  -3,
  -14,   2, -11,  -2,  -5,   2,  14,   5,
  -35,  -8,  11,   2,   8,  15,  -3,   1,
   -1, -18,  -9,  10, -15, -25, -31, -50,
];

#[rustfmt::skip]
const EG_QUEEN_TABLE: [Eval; 64] = [
   -9,  22,  22,  27,  27,  19,  10,  20,
  -17,  20,  32,  41,  58,  25,  30,   0,
  -20,   6,   9,  49,  47,  35,  19,   9,
    3,  22,  24,  45,  57,  40,  57,  36,
  -18,  28,  19,  47,  31,  34,  39,  23,
  -16, -27,  15,   6,   9,  17,  10,   5,
  -22, -23, -30, -16, -16, -23, -36, -32,
  -33, -28, -22, -43,  -5, -32, -20, -41,
];

#[rustfmt::skip]
const MG_KING_TABLE: [Eval; 64] = [
  -65,  23,  16, -15, -56, -34,   2,  13,
   29,  -1, -20,  -7,  -8,  -4, -38, -29,
   -9,  24,   2, -16, -20,   6,  22, -22,
  -17, -20, -12, -27, -30, -25, -14, -36,
  -49,  -1, -27, -39, -46, -44, -33, -51,
  -14, -14, -22, -46, -44, -30, -15, -27,
    1,   7,  -8, -64, -43, -16,   9,   8,
  -15,  36,  12, -54,   8, -28,  24,  14,
];

#[rustfmt::skip]
const EG_KING_TABLE: [Eval; 64] = [
  -74, -35, -18, -18, -11,  15,   4, -17,
  -12,  17,  14,  17,  17,  38,  23,  11,
   10,  17,  23,  15,  20,  45,  44,  13,
   -8,  22,  24,  27,  26,  33,  26,   3,
  -18,  -4,  21,  24,  27,  23,   9, -11,
  -19,  -3,  11,  21,  23,  16,   7,  -9,
  -27, -11,   4,  13,  14,   4,  -5, -17,
  -53, -34, -21, -11, -28, -14, -24, -43
];

pub type Eval = i16;

pub const EVAL_INFINITY: Eval = 30_000;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pawns() {
        let sq = cozy_chess::Square::E3;
        let pc = Color::White;

        let pawns_in_front = pawns_in_front_adjacent_files(sq, pc);

        assert_eq!(
            pawns_in_front,
            cozy_chess::bitboard!(
                . . . X X X . .
                . . . X X X . .
                . . . X X X . .
                . . . X X X . .
                . . . X X X . .
                . . . . . . . .
                . . . . . . . .
                . . . . . . . .
            )
        );
    }

    #[test]
    fn test_pawns_black() {
        let sq = cozy_chess::Square::D6;
        let pc = Color::Black;

        let pawns_in_front = pawns_in_front_adjacent_files(sq, pc);

        assert_eq!(
            pawns_in_front,
            cozy_chess::bitboard!(
                . . . . . . . .
                . . . . . . . .
                . . . . . . . .
                . . X X X . . .
                . . X X X . . .
                . . X X X . . .
                . . X X X . . .
                . . X X X . . .
            )
        );
    }

    #[test]
    fn test_pawns_side_white() {
        let sq = cozy_chess::Square::A2;
        let pc = Color::White;

        let pawns_in_front = pawns_in_front_adjacent_files(sq, pc);

        assert_eq!(
            pawns_in_front,
            cozy_chess::bitboard!(
                X X . . . . . .
                X X . . . . . .
                X X . . . . . .
                X X . . . . . .
                X X . . . . . .
                X X . . . . . .
                . . . . . . . .
                . . . . . . . .
            )
        );
    }

    #[test]
    fn test_pawns_side_white_far() {
        let sq = cozy_chess::Square::A7;
        let pc = Color::White;

        let pawns_in_front = pawns_in_front_adjacent_files(sq, pc);

        assert_eq!(
            pawns_in_front,
            cozy_chess::bitboard!(
                X X . . . . . .
                . . . . . . . .
                . . . . . . . .
                . . . . . . . .
                . . . . . . . .
                . . . . . . . .
                . . . . . . . .
                . . . . . . . .
            )
        );
    }

    #[test]
    fn test_pawns_side_white_right_far() {
        let sq = cozy_chess::Square::H7;
        let pc = Color::White;

        let pawns_in_front = pawns_in_front_adjacent_files(sq, pc);

        assert_eq!(
            pawns_in_front,
            cozy_chess::bitboard!(
                . . . . . . X X
                . . . . . . . .
                . . . . . . . .
                . . . . . . . .
                . . . . . . . .
                . . . . . . . .
                . . . . . . . .
                . . . . . . . .
            )
        );
    }

    #[test]
    fn test_pawns_side_black_far() {
        let sq = cozy_chess::Square::H2;
        let pc = Color::Black;

        let pawns_in_front = pawns_in_front_adjacent_files(sq, pc);

        assert_eq!(
            pawns_in_front,
            cozy_chess::bitboard!(
                . . . . . . . .
                . . . . . . . .
                . . . . . . . .
                . . . . . . . .
                . . . . . . . .
                . . . . . . . .
                . . . . . . . .
                . . . . . . X X
            )
        );
    }

    #[test]
    fn test_pawns_side_black_far_left() {
        let sq = cozy_chess::Square::A4;
        let pc = Color::Black;

        let pawns_in_front = pawns_in_front_adjacent_files(sq, pc);

        assert_eq!(
            pawns_in_front,
            cozy_chess::bitboard!(
                . . . . . . . .
                . . . . . . . .
                . . . . . . . .
                . . . . . . . .
                . . . . . . . .
                X X . . . . . .
                X X . . . . . .
                X X . . . . . .
            )
        );
    }

    #[test]
    fn test_pawns_side_black_far_left_b4() {
        let sq = cozy_chess::Square::B4;
        let pc = Color::Black;

        let pawns_in_front = pawns_in_front_adjacent_files(sq, pc);

        assert_eq!(
            pawns_in_front,
            cozy_chess::bitboard!(
                . . . . . . . .
                . . . . . . . .
                . . . . . . . .
                . . . . . . . .
                . . . . . . . .
                X X X . . . . .
                X X X . . . . .
                X X X . . . . .
            )
        );
    }

    #[test]
    fn test_pawns_side_black() {
        let sq = cozy_chess::Square::B5;
        let pc = Color::Black;

        let pawns_in_front = pawns_in_front_adjacent_files(sq, pc);

        assert_eq!(
            pawns_in_front,
            cozy_chess::bitboard!(
                . . . . . . . .
                . . . . . . . .
                . . . . . . . .
                . . . . . . . .
                X X X . . . . .
                X X X . . . . .
                X X X . . . . .
                X X X . . . . .
            )
        );
    }
}
