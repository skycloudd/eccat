#![deny(unsafe_code)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::std_instead_of_core)]
#![warn(clippy::alloc_instead_of_core)]
#![allow(clippy::inline_always)]
#![allow(clippy::module_name_repetitions)]

use cozy_chess::{util::parse_uci_move, Board};
use search::{EngineToSearch, History, Search, SearchMode, SearchToEngine};
use std::sync::{Arc, Mutex};
use uci::{EngineToUci, Uci, UciToEngine};

mod evaluate;
mod search;
mod tt;
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
    options: EngineOptions,
}

impl Engine {
    fn new() -> Self {
        Self {
            uci: Uci::new(),
            search: Search::new(),
            quit: false,
            debug: false,
            options: EngineOptions::default(),
        }
    }

    #[allow(clippy::too_many_lines)]
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
                        match name.to_ascii_lowercase().as_str() {
                            "hash" => match value {
                                Some(value) => {
                                    let value = value.parse().map_err(|_| {
                                        format!("invalid value for Hash option: {value}")
                                    });

                                    match value {
                                        Ok(value) => {
                                            if let Err(error) = self.options.hash.set(value) {
                                                eprintln!("error: {error}");
                                            }

                                            self.search.send(EngineToSearch::SetHash(
                                                usize::try_from(value).unwrap(),
                                            ));
                                        }
                                        Err(error) => {
                                            eprintln!("error: {error}");
                                        }
                                    }
                                }
                                None => {
                                    eprintln!("error: missing value for Hash option");
                                }
                            },
                            _ => {
                                eprintln!("warning: unsupported option: {name} = {value:?}");
                            }
                        }
                    }
                    UciToEngine::UciNewGame => {
                        *board.lock().unwrap() = Board::default();
                        *history.lock().unwrap() = Vec::new();

                        self.search.send(EngineToSearch::ClearHash);
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
                        let eval = evaluate::evaluate(&board.lock().unwrap());

                        let side_to_move = board.lock().unwrap().side_to_move();

                        println!(
                            "{}",
                            match side_to_move {
                                cozy_chess::Color::White => eval,
                                cozy_chess::Color::Black => -eval,
                            }
                        );
                    }
                    UciToEngine::PrintBoard => {
                        let board = board.lock().unwrap();

                        pretty_print_board(&board);

                        println!("{board}");
                        println!("hash: {:x}", board.hash());
                    }
                    UciToEngine::PrintOptions => {
                        println!("Options:");

                        println!(
                            "  {name} = {value}",
                            name = HashOption::name(),
                            value = self.options.hash.get()
                        );
                    }
                    UciToEngine::PlayMove(mv) => {
                        let parsed_move = parse_uci_move(&board.lock().unwrap(), &mv);

                        let mv = match parsed_move {
                            Ok(mv) => mv,
                            Err(err) => {
                                eprintln!("error: {err}");
                                continue;
                            }
                        };

                        let play_result = board.lock().unwrap().try_play(mv);

                        match play_result {
                            Ok(()) => {
                                let board = board.lock().unwrap();

                                history.lock().unwrap().push(History {
                                    hash: board.hash(),
                                    is_reversible_move: board.halfmove_clock() != 0,
                                });
                            }
                            Err(err) => {
                                eprintln!("error: {err}");
                            }
                        }
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
                        hashfull,
                        pv,
                    } => self.uci.send(EngineToUci::Summary {
                        depth,
                        seldepth,
                        time,
                        cp,
                        nodes,
                        nps,
                        hashfull,
                        pv,
                    }),
                },
                EngineReport::Error(error) => {
                    eprintln!("error: {error}");
                }
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
    Error(String),
}

struct EngineOptions {
    hash: HashOption,
}

impl Default for EngineOptions {
    fn default() -> Self {
        Self {
            hash: HashOption(HashOption::default()),
        }
    }
}

trait EngineOption {
    type Value;
    type Error;

    fn name() -> &'static str;
    fn min() -> Self::Value;
    fn max() -> Self::Value;
    fn default() -> Self::Value;

    fn get(&self) -> Self::Value;

    fn set(&mut self, value: Self::Value) -> Result<(), Self::Error>;
}

struct HashOption(pub i64);

impl EngineOption for HashOption {
    type Value = i64;
    type Error = String;

    fn name() -> &'static str {
        "Hash"
    }

    fn min() -> Self::Value {
        1
    }

    fn max() -> Self::Value {
        // what stockfish uses
        i64::MAX / 0x003F_FFFF_FFFF
    }

    fn default() -> Self::Value {
        16
    }

    fn get(&self) -> Self::Value {
        self.0
    }

    fn set(&mut self, value: Self::Value) -> Result<(), Self::Error> {
        if value < Self::min() {
            return Err(format!("{} must be at least {}", Self::name(), Self::min()));
        }

        if value > Self::max() {
            return Err(format!("{} must be at most {}", Self::name(), Self::max()));
        }

        self.0 = value;

        Ok(())
    }
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
