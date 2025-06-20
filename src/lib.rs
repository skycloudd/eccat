use crate::tt::TranspositionTable;
use cozy_chess::{Board, Color, File, Piece, Rank, Square, util::parse_uci_move};
use search::{EngineToSearch, History, Search, SearchMode, SearchToEngine};
use std::sync::{Arc, Mutex};
use uci::{EngineToUci, Uci, UciToEngine};

mod evaluate;
mod oracle;
mod search;
mod see;
mod tt;
mod uci;

const PKG_NAME: &str = env!("CARGO_PKG_NAME");
const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

const BUILD_DATE: &str = env!("VERGEN_BUILD_DATE");
const GIT_BRANCH: &str = env!("VERGEN_GIT_BRANCH");
const GIT_DESCRIBE: &str = env!("VERGEN_GIT_DESCRIBE");
const RUSTC_SEMVER: &str = env!("VERGEN_RUSTC_SEMVER");
const SYSINFO_NAME: &str = env!("VERGEN_SYSINFO_NAME");

const ERROR_VERGEN: &str = "VERGEN_IDEMPOTENT_OUTPUT";

const GIT_DESCRIBE_STR: &str = if const_str::equal!(GIT_DESCRIBE, ERROR_VERGEN) {
    ""
} else {
    const_str::format!(" ({GIT_DESCRIBE})")
};

const VERSION_STR: &str = const_str::format!("{PKG_NAME} v{PKG_VERSION}{GIT_DESCRIBE_STR}");

#[derive(Debug)]
pub struct Engine {
    uci: Uci,
    search: Search,
    quit: bool,
    debug: bool,
    options: EngineOptions,
}

impl Engine {
    #[must_use]
    pub fn new() -> Self {
        Self {
            uci: Uci::new(),
            search: Search::new(),
            quit: false,
            debug: false,
            options: EngineOptions::default(),
        }
    }

    #[allow(clippy::too_many_lines)]
    pub fn main_loop(&mut self) -> Result<(), Box<dyn core::error::Error>> {
        let (report_tx, report_rx) = crossbeam_channel::unbounded();

        let board = Arc::new(Mutex::new(Board::default()));
        let history = Arc::new(Mutex::new(Vec::new()));

        let transposition_table = Arc::new(Mutex::new(TranspositionTable::new(
            usize::try_from(HashOption::default()).unwrap(),
        )));

        self.uci.init(report_tx.clone());

        self.search.init(
            report_tx,
            Arc::clone(&board),
            Arc::clone(&history),
            Arc::clone(&transposition_table),
        );

        println!("{VERSION_STR} by {}", pkg_authors());

        println!(
            "({}{BUILD_DATE}) [Rust {RUSTC_SEMVER}] on {SYSINFO_NAME}",
            if GIT_BRANCH == ERROR_VERGEN {
                String::new()
            } else {
                format!("{GIT_BRANCH}, ")
            }
        );

        while !self.quit {
            match report_rx.recv()? {
                EngineReport::Uci(uci_report) => match uci_report {
                    UciToEngine::Uci => self.uci.send(EngineToUci::Identify)?,
                    UciToEngine::Debug(debug) => self.debug = debug,
                    UciToEngine::IsReady => self.uci.send(EngineToUci::Ready)?,
                    UciToEngine::Register => {
                        eprintln!("warning: register uci command not supported");
                    }
                    UciToEngine::Position(new_board, new_history) => {
                        *board.lock().unwrap() = new_board;
                        *history.lock().unwrap() = new_history;
                    }
                    UciToEngine::SetOption { name, value } => match name.to_lowercase().as_str() {
                        "hash" => match value {
                            Some(value) => match value.parse() {
                                Ok(value) => match self.options.hash.set(value) {
                                    Ok(()) => {
                                        self.search.send(EngineToSearch::SetHash(
                                            usize::try_from(value)?,
                                        ))?;
                                    }
                                    Err(error) => {
                                        eprintln!("error: {error}");
                                    }
                                },
                                Err(error) => {
                                    eprintln!("error: invalid value for Hash option: {error}");
                                }
                            },
                            None => {
                                eprintln!("error: missing value for Hash option");
                            }
                        },
                        "threads" => match value {
                            Some(value) => match value.parse() {
                                Ok(value) => match self.options.threads.set(value) {
                                    Ok(()) => {}
                                    Err(error) => {
                                        eprintln!("error: {error}");
                                    }
                                },
                                Err(error) => {
                                    eprintln!("error: invalid value for Threads option: {error}");
                                }
                            },
                            None => {
                                eprintln!("error: missing value for Threads option");
                            }
                        },
                        _ => {
                            eprintln!("warning: unsupported option: {name} = {value:?}");
                        }
                    },
                    UciToEngine::UciNewGame => {
                        *board.lock().unwrap() = Board::default();
                        *history.lock().unwrap() = Vec::new();

                        self.search.send(EngineToSearch::ClearHash)?;
                    }
                    UciToEngine::Stop => self.search.send(EngineToSearch::Stop)?,
                    UciToEngine::PonderHit => {
                        eprintln!("warning: ponderhit uci command not supported");
                    }
                    UciToEngine::Quit => self.quit()?,
                    UciToEngine::GoInfinite => self
                        .search
                        .send(EngineToSearch::Start(SearchMode::Infinite))?,
                    UciToEngine::GoMoveTime(movetime) => self
                        .search
                        .send(EngineToSearch::Start(SearchMode::MoveTime(movetime)))?,
                    UciToEngine::GoGameTime(gametime) => self
                        .search
                        .send(EngineToSearch::Start(SearchMode::GameTime(gametime)))?,
                    UciToEngine::GoDepth(depth) => self
                        .search
                        .send(EngineToSearch::Start(SearchMode::Depth(depth)))?,

                    UciToEngine::Unknown(error) => {
                        if let Some(error) = error {
                            eprintln!("error: {error}");
                        }
                    }

                    UciToEngine::Eval => {
                        println!("side to move: {}", board.lock().unwrap().side_to_move());
                        println!(
                            "evaluation:   {}",
                            evaluate::evaluate(&board.lock().unwrap())
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

                        println!(
                            "  {name} = {value}",
                            name = ThreadsOption::name(),
                            value = 1
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

                                history.lock().unwrap().push(History { hash: board.hash() });
                            }
                            Err(err) => {
                                eprintln!("error: {err}");
                            }
                        }
                    }
                    UciToEngine::Help => {
                        println!("Custom commands:");
                        println!("  eval    - evaluate the current position");
                        println!("  board   - display the current board");
                        println!("  options - display the current engine options");
                        println!("  make    - make a move on the board (e.g. make e2e4)");
                        println!(
                            "  sleep   - sleep the uci thread for a number of milliseconds (e.g. sleep 1000)"
                        );
                        println!(
                            "  probe   - probe the transposition table for the current position"
                        );
                    }
                    UciToEngine::Sleep(ms) => {
                        println!("slept for {ms} ms");
                    }
                    UciToEngine::Probe => {
                        let key = board.lock().unwrap().hash();

                        if let Some(entry) = transposition_table.lock().unwrap().probe(key) {
                            let info = entry.info();

                            println!("found entry for this position");

                            println!("key: {}", info.key);
                            println!("depth: {}", info.depth);
                            println!("flag: {:?}", info.flag);
                            println!("score: {}", info.score);

                            if let Some(best_move) = info.best_move {
                                println!("best move: {best_move}");
                            }
                        } else {
                            println!("no entry found for this position with hash {key:x}");
                        }
                    }
                },
                EngineReport::Search(search_report) => match search_report {
                    SearchToEngine::BestMove(bestmove) => {
                        self.uci.send(EngineToUci::BestMove(bestmove))?;
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
                    })?,
                },
                EngineReport::Error(error) => {
                    eprintln!("error: {error}");
                }
            }
        }

        Ok(())
    }

    fn quit(&mut self) -> Result<(), Box<dyn core::error::Error>> {
        self.uci.send(EngineToUci::Quit)?;
        self.search.send(EngineToSearch::Quit)?;

        self.quit = true;

        Ok(())
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub enum EngineReport {
    Uci(UciToEngine),
    Search(SearchToEngine),
    Error(String),
}

#[derive(Debug)]
struct EngineOptions {
    hash: HashOption,
    threads: ThreadsOption,
}

impl Default for EngineOptions {
    fn default() -> Self {
        Self {
            hash: HashOption(HashOption::default()),
            threads: ThreadsOption(ThreadsOption::default()),
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

#[derive(Debug)]
struct HashOption(pub i64);

#[derive(Debug)]
struct ThreadsOption(pub i64);

macro_rules! impl_option {
    ($option:ty, $name:expr_2021, $value:ty, $min:expr_2021, $max:expr_2021, $default:expr_2021) => {
        impl EngineOption for $option {
            type Value = $value;
            type Error = String;

            fn name() -> &'static str {
                $name
            }

            fn min() -> Self::Value {
                $min
            }

            fn max() -> Self::Value {
                $max
            }

            fn default() -> Self::Value {
                $default
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
    };
}

impl_option!(
    HashOption,
    "Hash",
    i64,
    1,
    i64::from(u32::MAX) * 64 / (1024 * 1024),
    16
);

impl_option!(ThreadsOption, "Threads", i64, 1, 1, 1);

fn pkg_authors() -> String {
    env!("CARGO_PKG_AUTHORS")
        .split(':')
        .collect::<Vec<_>>()
        .join(", ")
}

fn pretty_print_board(board: &Board) {
    println!("+---+---+---+---+---+---+---+---+");

    for rank in Rank::ALL.into_iter().rev() {
        print!("|");

        for file in File::ALL {
            let square = Square::new(file, rank);

            let piece = board.piece_on(square);

            let colour = board.color_on(square);

            match (piece, colour) {
                (Some(piece), Some(colour)) => {
                    let symbol = match piece {
                        Piece::Pawn => 'p',
                        Piece::Knight => 'n',
                        Piece::Bishop => 'b',
                        Piece::Rook => 'r',
                        Piece::Queen => 'q',
                        Piece::King => 'k',
                    };

                    let symbol = match colour {
                        Color::White => symbol.to_ascii_uppercase(),
                        Color::Black => symbol,
                    };

                    print!(" {symbol} |");
                }
                _ => print!("   |"),
            }
        }

        println!("\n+---+---+---+---+---+---+---+---+");
    }
}
