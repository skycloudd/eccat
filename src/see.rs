use crate::{evaluate::Eval, search::is_capture};
use cozy_chess::{
    get_bishop_moves, get_king_moves, get_knight_moves, get_pawn_attacks, get_rook_moves, Board,
    Color, Piece,
};

// thanks to https://github.com/analog-hors/tantabus ♡
pub fn see(board: &Board, capture: cozy_chess::Move) -> Eval {
    debug_assert!(is_capture(board, capture));

    let target_square = capture.to;
    let initial_capture = board.piece_on(target_square).unwrap();
    let initial_colour = board.side_to_move();

    let mut blockers = board.occupied() ^ capture.from.bitboard();

    let mut attackers = get_king_moves(target_square) & blockers & board.pieces(Piece::King)
        | get_knight_moves(target_square) & blockers & board.pieces(Piece::Knight)
        | get_rook_moves(target_square, blockers)
            & blockers
            & (board.pieces(Piece::Rook) | board.pieces(Piece::Queen))
        | get_bishop_moves(target_square, blockers)
            & blockers
            & (board.pieces(Piece::Bishop) | board.pieces(Piece::Queen))
        | get_pawn_attacks(target_square, Color::Black)
            & blockers
            & board.colored_pieces(Color::White, Piece::Pawn)
        | get_pawn_attacks(target_square, Color::White)
            & blockers
            & board.colored_pieces(Color::Black, Piece::Pawn);

    let mut target_piece = board.piece_on(capture.from).unwrap();
    let mut colour = !initial_colour;

    let mut gains = vec![piece_value(initial_capture)];

    'exchange: loop {
        for attacker_piece in Piece::ALL {
            let our_attacker = attackers & board.colored_pieces(colour, attacker_piece);

            if let Some(attacker_square) = our_attacker.next_square() {
                let victim_value = piece_value(target_piece);
                gains.push(victim_value);

                if target_piece == Piece::King {
                    break;
                }

                blockers ^= attacker_square.bitboard();
                attackers ^= attacker_square.bitboard();

                target_piece = attacker_piece;

                if matches!(attacker_piece, Piece::Rook | Piece::Queen) {
                    attackers |= get_rook_moves(target_square, blockers)
                        & blockers
                        & (board.pieces(Piece::Rook) | board.pieces(Piece::Queen));
                }

                if matches!(attacker_piece, Piece::Pawn | Piece::Bishop | Piece::Queen) {
                    attackers |= get_bishop_moves(target_square, blockers)
                        & blockers
                        & (board.pieces(Piece::Bishop) | board.pieces(Piece::Queen));
                }

                colour = !colour;

                continue 'exchange;
            }
        }

        while gains.len() > 1 {
            let forced = gains.len() == 2;

            let their_gain = gains.pop().unwrap();
            let our_gain = gains.last_mut().unwrap();

            *our_gain -= their_gain;

            if !forced && *our_gain < 0 {
                *our_gain = 0;
            }
        }

        return gains.pop().unwrap();
    }
}

const fn piece_value(piece: Piece) -> Eval {
    match piece {
        Piece::Pawn => 100,
        Piece::Knight => 320,
        Piece::Bishop => 330,
        Piece::Rook => 500,
        Piece::Queen => 900,
        Piece::King => 10000,
    }
}
