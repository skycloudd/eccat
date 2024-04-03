use cozy_chess::{Board, Color, Piece, Square};

pub fn evaluate(board: &Board) -> Eval {
    let mut eval = Eval(0);

    let is_endgame = is_endgame(board);

    for square in board.occupied() {
        let piece = board.piece_on(square).unwrap();
        let piece_colour = board.color_on(square).unwrap();

        let value = match piece {
            Piece::Pawn => 100,
            Piece::Knight => 320,
            Piece::Bishop => 330,
            Piece::Rook => 500,
            Piece::Queen => 900,
            Piece::King => 20000,
        };

        eval += Eval(match piece_colour {
            Color::White => value,
            Color::Black => -value,
        });

        eval += piece_square(piece, piece_colour, square, is_endgame);
    }

    match board.side_to_move() {
        Color::White => eval,
        Color::Black => -eval,
    }
}

const fn piece_square(piece: Piece, piece_colour: Color, square: Square, is_endgame: bool) -> Eval {
    let table = match piece {
        Piece::Pawn => PAWN_TABLE,
        Piece::Knight => KNIGHT_TABLE,
        Piece::Bishop => BISHOP_TABLE,
        Piece::Rook => ROOK_TABLE,
        Piece::Queen => QUEEN_TABLE,
        Piece::King => {
            if is_endgame {
                KING_TABLE_ENDGAME
            } else {
                KING_TABLE
            }
        }
    };

    let index = match piece_colour {
        Color::White => 63 - square as usize,
        Color::Black => square as usize,
    };

    Eval(table[index])
}

fn is_endgame(board: &Board) -> bool {
    if board.pieces(Piece::Queen).is_empty() {
        true
    } else {
        let white_knights = board.colored_pieces(Color::White, Piece::Knight).len();
        let white_bishops = board.colored_pieces(Color::White, Piece::Bishop).len();
        let white_rooks = board.colored_pieces(Color::White, Piece::Rook).len();

        let white_endgame = (white_knights + white_bishops <= 1) && white_rooks == 0;

        let black_knights = board.colored_pieces(Color::Black, Piece::Knight).len();
        let black_bishops = board.colored_pieces(Color::Black, Piece::Bishop).len();
        let black_rooks = board.colored_pieces(Color::Black, Piece::Rook).len();

        let black_endgame = (black_knights + black_bishops <= 1) && black_rooks == 0;

        white_endgame && black_endgame
    }
}

const PAWN_TABLE: [i16; 64] = [
    0, 0, 0, 0, 0, 0, 0, 0, 50, 50, 50, 50, 50, 50, 50, 50, 10, 10, 20, 30, 30, 20, 10, 10, 5, 5,
    10, 25, 25, 10, 5, 5, 0, 0, 0, 20, 20, 0, 0, 0, 5, -5, -10, 0, 0, -10, -5, 5, 5, 10, 10, -20,
    -20, 10, 10, 5, 0, 0, 0, 0, 0, 0, 0, 0,
];

const KNIGHT_TABLE: [i16; 64] = [
    -50, -40, -30, -30, -30, -30, -40, -50, -40, -20, 0, 0, 0, 0, -20, -40, -30, 0, 10, 15, 15, 10,
    0, -30, -30, 5, 15, 20, 20, 15, 5, -30, -30, 0, 15, 20, 20, 15, 0, -30, -30, 5, 10, 15, 15, 10,
    5, -30, -40, -20, 0, 5, 5, 0, -20, -40, -50, -40, -30, -30, -30, -30, -40, -50,
];

const BISHOP_TABLE: [i16; 64] = [
    -20, -10, -10, -10, -10, -10, -10, -20, -10, 0, 0, 0, 0, 0, 0, -10, -10, 0, 5, 10, 10, 5, 0,
    -10, -10, 5, 5, 10, 10, 5, 5, -10, -10, 0, 10, 10, 10, 10, 0, -10, -10, 10, 10, 10, 10, 10, 10,
    -10, -10, 5, 0, 0, 0, 0, 5, -10, -20, -10, -10, -10, -10, -10, -10, -20,
];

const ROOK_TABLE: [i16; 64] = [
    0, 0, 0, 0, 0, 0, 0, 0, 5, 10, 10, 10, 10, 10, 10, 5, -5, 0, 0, 0, 0, 0, 0, -5, -5, 0, 0, 0, 0,
    0, 0, -5, -5, 0, 0, 0, 0, 0, 0, -5, -5, 0, 0, 0, 0, 0, 0, -5, -5, 0, 0, 0, 0, 0, 0, -5, 0, 0,
    0, 5, 5, 0, 0, 0,
];

const QUEEN_TABLE: [i16; 64] = [
    -20, -10, -10, -5, -5, -10, -10, -20, -10, 0, 0, 0, 0, 0, 0, -10, -10, 0, 5, 5, 5, 5, 0, -10,
    -5, 0, 5, 5, 5, 5, 0, -5, 0, 0, 5, 5, 5, 5, 0, -5, -10, 5, 5, 5, 5, 5, 0, -10, -10, 0, 5, 0, 0,
    0, 0, -10, -20, -10, -10, -5, -5, -10, -10, -20,
];

const KING_TABLE: [i16; 64] = [
    -30, -40, -40, -50, -50, -40, -40, -30, -30, -40, -40, -50, -50, -40, -40, -30, -30, -40, -40,
    -50, -50, -40, -40, -30, -30, -40, -40, -50, -50, -40, -40, -30, -20, -30, -30, -40, -40, -30,
    -30, -20, -10, -20, -20, -20, -20, -20, -20, -10, 20, 20, 0, 0, 0, 0, 20, 20, 20, 30, 10, 0, 0,
    10, 30, 20,
];

const KING_TABLE_ENDGAME: [i16; 64] = [
    -50, -40, -30, -20, -20, -30, -40, -50, -30, -20, -10, 0, 0, -10, -20, -30, -30, -10, 20, 30,
    30, 20, -10, -30, -30, -10, 30, 40, 40, 30, -10, -30, -30, -10, 30, 40, 40, 30, -10, -30, -30,
    -10, 20, 30, 30, 20, -10, -30, -30, -30, 0, 0, 0, 0, -30, -30, -50, -30, -30, -30, -30, -30,
    -30, -50,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Eval(pub i16);

impl Eval {
    pub const INFINITY: Self = Self(10000);
}

impl core::ops::Deref for Eval {
    type Target = i16;

    fn deref(&self) -> &i16 {
        &self.0
    }
}

impl core::ops::DerefMut for Eval {
    fn deref_mut(&mut self) -> &mut i16 {
        &mut self.0
    }
}

impl core::ops::Neg for Eval {
    type Output = Self;

    fn neg(self) -> Self {
        Self(-self.0)
    }
}

impl core::ops::AddAssign for Eval {
    fn add_assign(&mut self, other: Self) {
        *self = Self(self.0 + other.0);
    }
}
