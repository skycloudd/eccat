#![deny(unsafe_code)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![allow(clippy::module_name_repetitions)]
#![warn(clippy::std_instead_of_core)]
#![warn(clippy::alloc_instead_of_core)]

use core::str::FromStr;
use cozy_chess::Board;
use search::{EngineToSearch, History, Search, SearchMode, SearchToEngine};
use std::sync::{Arc, Mutex};
use uci::{EngineToUci, Uci, UciToEngine};

mod evaluate;
mod search;
mod uci;

fn main() {
    Engine::new().main_loop();
}

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
                    UciToEngine::Register => unimplemented!(),
                    UciToEngine::Position(fen, moves) => {
                        let mut board = board.lock().unwrap();
                        let mut history = history.lock().unwrap();

                        *board = Board::from_str(&fen).unwrap();

                        *history = Vec::new();

                        for m in moves {
                            board.play(m);

                            history.push(History {
                                hash: board.hash(),
                                is_reversible_move: board.halfmove_clock() != 0,
                            });
                        }
                    }
                    UciToEngine::SetOption => unimplemented!(),
                    UciToEngine::UciNewGame => {
                        *board.lock().unwrap() = Board::default();
                        *history.lock().unwrap() = Vec::new();
                    }
                    UciToEngine::Stop => self.search.send(EngineToSearch::Stop),
                    UciToEngine::PonderHit => unimplemented!(),
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
                    UciToEngine::Unknown => {}
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
