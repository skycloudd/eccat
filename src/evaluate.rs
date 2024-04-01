use cozy_chess::{Board, Color, Piece};
use std::ops::{Deref, DerefMut};

pub fn evaluate(board: &Board) -> Eval {
    let mut eval = 0;

    let pieces = [
        (Piece::Pawn, 100),
        (Piece::Knight, 320),
        (Piece::Bishop, 330),
        (Piece::Rook, 500),
        (Piece::Queen, 900),
        (Piece::King, 20000),
    ];

    for (piece, value) in pieces {
        let white = i16::try_from(board.colored_pieces(Color::White, piece).len()).unwrap();
        let black = i16::try_from(board.colored_pieces(Color::Black, piece).len()).unwrap();

        eval += (white - black) * value;
    }

    Eval(match board.side_to_move() {
        Color::White => eval,
        Color::Black => -eval,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Eval(pub i16);

impl Eval {
    pub const INFINITY: Self = Self(10000);
}

impl Deref for Eval {
    type Target = i16;

    fn deref(&self) -> &i16 {
        &self.0
    }
}

impl DerefMut for Eval {
    fn deref_mut(&mut self) -> &mut i16 {
        &mut self.0
    }
}

impl std::ops::Neg for Eval {
    type Output = Self;

    fn neg(self) -> Self {
        Self(-self.0)
    }
}
