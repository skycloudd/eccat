use cozy_chess::{Board, BoardBuilder, BoardBuilderError, Color, Piece, Rank, Square};

pub fn random_board() -> Board {
    let mut rng = rand::thread_rng();

    loop {
        if let Ok(board) = try_random_board(&mut rng) {
            if board.checkers().is_empty() {
                return board;
            }
        }
    }
}

fn try_random_board(rng: &mut impl rand::Rng) -> Result<Board, BoardBuilderError> {
    let mut builder = BoardBuilder::empty();

    loop {
        let king_white_square = random_square_without_piece(rng, &builder);
        let king_black_square = random_square_without_piece(rng, &builder);

        if !squares_touching(king_white_square, king_black_square) {
            set_square(&mut builder, king_white_square, (Piece::King, Color::White));

            set_square(&mut builder, king_black_square, (Piece::King, Color::Black));

            break;
        }
    }

    for _ in 0..rng.gen_range(0..=1) {
        let square = random_square_without_piece(rng, &builder);

        set_square(&mut builder, square, (Piece::Queen, Color::White));
    }

    for _ in 0..rng.gen_range(0..=1) {
        let square = random_square_without_piece(rng, &builder);

        set_square(&mut builder, square, (Piece::Queen, Color::Black));
    }

    for _ in 0..rng.gen_range(0..=2) {
        let square = random_square_without_piece(rng, &builder);

        set_square(&mut builder, square, (Piece::Rook, Color::White));
    }

    for _ in 0..rng.gen_range(0..=2) {
        let square = random_square_without_piece(rng, &builder);

        set_square(&mut builder, square, (Piece::Rook, Color::Black));
    }

    for _ in 0..rng.gen_range(0..=2) {
        let square = random_square_without_piece(rng, &builder);

        set_square(&mut builder, square, (Piece::Bishop, Color::White));
    }

    for _ in 0..rng.gen_range(0..=2) {
        let square = random_square_without_piece(rng, &builder);

        set_square(&mut builder, square, (Piece::Bishop, Color::Black));
    }

    for _ in 0..rng.gen_range(0..=2) {
        let square = random_square_without_piece(rng, &builder);

        set_square(&mut builder, square, (Piece::Knight, Color::White));
    }

    for _ in 0..rng.gen_range(0..=2) {
        let square = random_square_without_piece(rng, &builder);

        set_square(&mut builder, square, (Piece::Knight, Color::Black));
    }

    for _ in 0..rng.gen_range(0..=7) {
        let square = random_square_without_piece(rng, &builder);

        if square.rank() == Rank::First || square.rank() == Rank::Eighth {
            continue;
        }

        set_square(&mut builder, square, (Piece::Pawn, Color::White));
    }

    for _ in 0..rng.gen_range(0..=7) {
        let square = random_square_without_piece(rng, &builder);

        if square.rank() == Rank::First || square.rank() == Rank::Eighth {
            continue;
        }

        set_square(&mut builder, square, (Piece::Pawn, Color::Black));
    }

    if rng.gen_bool(0.5) {
        builder.side_to_move = Color::White;
    } else {
        builder.side_to_move = Color::Black;
    }

    builder.build()
}

fn set_square(builder: &mut BoardBuilder, square: Square, piece: (Piece, Color)) {
    *builder.square_mut(square) = Some(piece);
}

fn random_square_without_piece(rng: &mut impl rand::Rng, board: &BoardBuilder) -> Square {
    loop {
        let square = random_square(rng);

        if board.square(square).is_none() {
            return square;
        }
    }
}

fn random_square(rng: &mut impl rand::Rng) -> Square {
    Square::index(rng.gen_range(0..64))
}

const fn squares_touching(first: Square, second: Square) -> bool {
    let first_file = first.file();
    let first_rank = first.rank();

    let second_file = second.file();
    let second_rank = second.rank();

    let file_diff = (first_file as i8 - second_file as i8).abs();
    let rank_diff = (first_rank as i8 - second_rank as i8).abs();

    file_diff <= 1 && rank_diff <= 1
}
