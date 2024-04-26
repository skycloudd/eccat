#![deny(unsafe_code)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::std_instead_of_core)]
#![warn(clippy::alloc_instead_of_core)]
#![allow(clippy::inline_always)]
#![allow(clippy::module_name_repetitions)]

use cozy_chess::Board;
use search::{EngineToSearch, Search, SearchMode, SearchToEngine};
use std::sync::{Arc, Mutex};
use uci::{EngineToUci, Uci, UciToEngine};

mod evaluate;
mod search;
mod uci;

fn main() {
    Engine::new().main_loop();
}

#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

struct Engine {
    uci: Uci,
    search: Search,
    quit: bool,
    debug: bool,
}

impl Engine {
    fn new() -> Self {
        Self {
            uci: Uci::new(),
            search: Search::new(),
            quit: false,
            debug: false,
        }
    }

    fn main_loop(&mut self) {
        let (report_tx, report_rx) = crossbeam_channel::unbounded();

        let board = Arc::new(Mutex::new(Board::default()));
        let history = Arc::new(Mutex::new(Vec::new()));

        self.uci.init(report_tx.clone());

        self.search
            .init(report_tx, Arc::clone(&board), Arc::clone(&history));

        while !self.quit {
            match report_rx.recv().unwrap() {
                EngineReport::Uci(uci_report) => match uci_report {
                    UciToEngine::Uci => self.uci.send(EngineToUci::Identify),
                    UciToEngine::Debug(debug) => self.debug = debug,
                    UciToEngine::IsReady => self.uci.send(EngineToUci::Ready),
                    UciToEngine::Register => {
                        eprintln!("warning: register uci command not supported");
                    }
                    UciToEngine::Position(new_board, new_history) => {
                        *board.lock().unwrap() = new_board;
                        *history.lock().unwrap() = new_history;
                    }
                    UciToEngine::SetOption { name, value } => {
                        eprintln!("warning: unsupported option: {name} = {value:?}");
                    }
                    UciToEngine::UciNewGame => {
                        *board.lock().unwrap() = Board::default();
                        *history.lock().unwrap() = Vec::new();
                    }
                    UciToEngine::Stop => self.search.send(EngineToSearch::Stop),
                    UciToEngine::PonderHit => {
                        eprintln!("warning: ponderhit uci command not supported");
                    }
                    UciToEngine::Quit => self.quit(),
                    UciToEngine::GoInfinite => self
                        .search
                        .send(EngineToSearch::Start(SearchMode::Infinite)),
                    UciToEngine::GoMoveTime(movetime) => self
                        .search
                        .send(EngineToSearch::Start(SearchMode::MoveTime(movetime))),
                    UciToEngine::GoGameTime(gametime) => self
                        .search
                        .send(EngineToSearch::Start(SearchMode::GameTime(gametime))),

                    UciToEngine::Unknown(error) => {
                        if let Some(error) = error {
                            eprintln!("error: {error}");
                        }
                    }

                    UciToEngine::Eval => {
                        println!("{}", evaluate::evaluate(&board.lock().unwrap()));
                    }
                    UciToEngine::PrintBoard => {
                        let board = board.lock().unwrap();

                        pretty_print_board(&board);

                        println!("{board}");
                        println!("hash: {:x}", board.hash());
                    }
                },
                EngineReport::Search(search_report) => match search_report {
                    SearchToEngine::BestMove(bestmove) => {
                        self.uci.send(EngineToUci::BestMove(bestmove));
                    }
                    search::SearchToEngine::Summary {
                        depth,
                        seldepth,
                        time,
                        cp,
                        nodes,
                        nps,
                        pv,
                    } => self.uci.send(EngineToUci::Summary {
                        depth,
                        seldepth,
                        time,
                        cp,
                        nodes,
                        nps,
                        pv,
                    }),
                },
            }
        }
    }

    fn quit(&mut self) {
        self.uci.send(EngineToUci::Quit);
        self.search.send(EngineToSearch::Quit);

        self.quit = true;
    }
}

pub enum EngineReport {
    Uci(UciToEngine),
    Search(SearchToEngine),
}

fn pretty_print_board(board: &Board) {
    println!("+---+---+---+---+---+---+---+---+");

    for rank in cozy_chess::Rank::ALL.into_iter().rev() {
        print!("|");

        for file in cozy_chess::File::ALL {
            let square = cozy_chess::Square::new(file, rank);

            let piece = board.piece_on(square);

            let colour = board.color_on(square);

            match (piece, colour) {
                (Some(piece), Some(colour)) => {
                    let symbol = match piece {
                        cozy_chess::Piece::Pawn => 'p',
                        cozy_chess::Piece::Knight => 'n',
                        cozy_chess::Piece::Bishop => 'b',
                        cozy_chess::Piece::Rook => 'r',
                        cozy_chess::Piece::Queen => 'q',
                        cozy_chess::Piece::King => 'k',
                    };

                    let symbol = match colour {
                        cozy_chess::Color::White => symbol.to_ascii_uppercase(),
                        cozy_chess::Color::Black => symbol,
                    };

                    print!(" {symbol} |");
                }
                _ => print!("   |"),
            }
        }

        println!("\n+---+---+---+---+---+---+---+---+");
    }
}
