use crate::{evaluate::Eval, search::History, EngineReport};
use chrono::Duration;
use core::{fmt::Display, str::FromStr};
use cozy_chess::{
    util::{display_uci_move, parse_uci_move},
    Board, Move, MoveParseError,
};
use crossbeam_channel::Sender;
use std::thread::JoinHandle;
use vampirc_uci::{UciInfoAttribute, UciMessage, UciMove, UciTimeControl};

pub enum EngineToUci {
    Identify,
    Ready,
    Quit,
    BestMove(String),
    Summary {
        depth: u8,
        seldepth: u8,
        time: Duration,
        cp: Eval,
        nodes: u64,
        nps: u64,
        pv: Vec<String>,
    },
}

pub enum UciToEngine {
    Uci,
    Debug(bool),
    IsReady,
    Register,
    Position(Board, Vec<History>),
    SetOption,
    UciNewGame,
    Stop,
    PonderHit,
    Quit,
    GoInfinite,
    GoMoveTime(Duration),
    GoGameTime(GameTime),
    Unknown,
}

#[derive(Debug, Default)]
pub struct Uci {
    report_handle: Option<JoinHandle<()>>,
    control_handle: Option<JoinHandle<()>>,
    control_tx: Option<Sender<EngineToUci>>,
}

impl Uci {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn init(&mut self, report_tx: Sender<EngineReport>) {
        self.report_thread(report_tx);
        self.control_thread();
    }

    pub fn send(&mut self, msg: EngineToUci) {
        if let Some(tx) = &self.control_tx {
            tx.send(msg).unwrap();
        }
    }

    fn report_thread(&mut self, report_tx: Sender<EngineReport>) {
        let mut incoming_data = String::new();

        let report_handle = std::thread::spawn(move || {
            let mut quit = false;

            while !quit {
                std::io::stdin().read_line(&mut incoming_data).unwrap();

                let msgs = vampirc_uci::parse(&incoming_data);

                for msg in msgs {
                    let report = Self::handle_msg(msg);

                    if matches!(report, UciToEngine::Quit) {
                        quit = true;
                    }

                    report_tx.send(EngineReport::Uci(report)).unwrap();
                }

                incoming_data.clear();
            }
        });

        self.report_handle = Some(report_handle);
    }

    fn handle_msg(msg: UciMessage) -> UciToEngine {
        match msg {
            UciMessage::Uci => UciToEngine::Uci,

            UciMessage::Debug(debug) => UciToEngine::Debug(debug),

            UciMessage::IsReady => UciToEngine::IsReady,

            UciMessage::Register {
                later: _,
                name: _,
                code: _,
            } => UciToEngine::Register,

            UciMessage::Position {
                startpos,
                fen,
                moves,
            } => {
                let fen = if startpos {
                    String::from("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")
                } else {
                    fen.unwrap().to_string()
                };

                let mut board = Board::from_str(&fen).unwrap();
                let mut history = Vec::with_capacity(moves.len());

                for m in &moves {
                    board.play(convert_move_from_uci(&board, m).unwrap());

                    history.push(History {
                        hash: board.hash(),
                        is_reversible_move: board.halfmove_clock() != 0,
                    });
                }

                UciToEngine::Position(board, history)
            }

            UciMessage::SetOption { name: _, value: _ } => UciToEngine::SetOption,

            UciMessage::UciNewGame => UciToEngine::UciNewGame,

            UciMessage::Stop => UciToEngine::Stop,

            UciMessage::PonderHit => UciToEngine::PonderHit,

            UciMessage::Quit => UciToEngine::Quit,

            UciMessage::Go {
                time_control,
                search_control,
            } => time_control.map_or_else(
                || {
                    search_control.map_or_else(
                        || unreachable!(),
                        |search_control| todo!("{:?}", search_control),
                    )
                },
                |time_control| match time_control {
                    UciTimeControl::Ponder => unimplemented!(),
                    UciTimeControl::Infinite => UciToEngine::GoInfinite,
                    UciTimeControl::TimeLeft {
                        white_time,
                        black_time,
                        white_increment,
                        black_increment,
                        moves_to_go,
                    } => UciToEngine::GoGameTime(GameTime {
                        white_time: white_time.unwrap_or_default(),
                        black_time: black_time.unwrap_or_default(),
                        white_increment: white_increment.unwrap_or_default(),
                        black_increment: black_increment.unwrap_or_default(),
                        moves_to_go,
                    }),
                    UciTimeControl::MoveTime(movetime) => UciToEngine::GoMoveTime(movetime),
                },
            ),

            _ => UciToEngine::Unknown,
        }
    }

    fn control_thread(&mut self) {
        let (control_tx, control_rx) = crossbeam_channel::unbounded();

        let control_handle = std::thread::spawn(move || {
            let mut quit = false;

            while !quit {
                let msg = control_rx.recv().unwrap();

                match msg {
                    EngineToUci::Identify => {
                        println!("{}", UciMessage::id_name("eccat"));
                        println!("{}", UciMessage::id_author("skycloudd"));
                        println!("{}", UciMessage::UciOk);
                    }
                    EngineToUci::Ready => println!("{}", UciMessage::ReadyOk),
                    EngineToUci::Quit => quit = true,
                    EngineToUci::BestMove(bestmove) => {
                        println!("bestmove {bestmove}");
                    }
                    EngineToUci::Summary {
                        depth,
                        seldepth,
                        time,
                        cp,
                        nodes,
                        nps,
                        pv,
                    } => {
                        let score = if cp.abs() > *Eval::INFINITY / 2 {
                            let mate_in_plies = *Eval::INFINITY - cp.abs();
                            let sign = cp.signum();

                            let mate_in_moves = mate_in_plies / 2 + mate_in_plies % 2;

                            UciInfoAttribute::from_mate((mate_in_moves * sign).try_into().unwrap())
                        } else {
                            UciInfoAttribute::from_centipawns(cp.0.into())
                        };

                        println!(
                            "{}{}",
                            UciMessage::Info(vec![
                                UciInfoAttribute::Depth(depth),
                                UciInfoAttribute::SelDepth(seldepth),
                                UciInfoAttribute::Time(time),
                                score,
                                UciInfoAttribute::Nodes(nodes),
                                UciInfoAttribute::Nps(nps),
                            ]),
                            if pv.is_empty() {
                                String::new()
                            } else {
                                format!(
                                    " pv {}",
                                    pv.iter()
                                        .map(ToString::to_string)
                                        .collect::<Vec<_>>()
                                        .join(" ")
                                )
                            }
                        );
                    }
                }
            }
        });

        self.control_handle = Some(control_handle);
        self.control_tx = Some(control_tx);
    }
}

#[derive(Debug)]
pub struct GameTime {
    pub white_time: Duration,
    pub black_time: Duration,
    pub white_increment: Duration,
    pub black_increment: Duration,
    pub moves_to_go: Option<u8>,
}

pub fn convert_move_from_uci(board: &Board, m: &UciMove) -> Result<Move, MoveParseError> {
    parse_uci_move(board, &m.to_string())
}

pub fn convert_move_to_uci(board: &Board, m: Move) -> impl Display {
    display_uci_move(board, m)
}
