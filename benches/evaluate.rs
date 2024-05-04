use criterion::{black_box, criterion_group, criterion_main, Criterion};
use eccat::evaluate;
use std::str::FromStr as _;

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("eval startpos", |b| {
        b.iter(|| {
            let board = cozy_chess::Board::startpos();

            black_box(evaluate::evaluate(&board))
        })
    });

    c.bench_function("eval midgame", |b| {
        b.iter(|| {
            let board = cozy_chess::Board::from_str(
                "r2q2k1/1p1n1rp1/1bp1p2p/1P1np3/2N5/B1PP1N1P/2Q2PP1/R3R1K1 w - - 2 19",
            )
            .unwrap();

            black_box(evaluate::evaluate(&board))
        })
    });

    c.bench_function("eval endgame", |b| {
        b.iter(|| {
            let board = cozy_chess::Board::from_str("6k1/8/4r3/3n1pB1/8/2P2P1P/6PK/1R6 w - - 3 37")
                .unwrap();

            black_box(evaluate::evaluate(&board))
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
