use crate::{
    evaluate::{Eval, EVAL_INFINITY},
    search::History,
    EngineOption as _, EngineReport, HashOption,
};
use chrono::Duration;
use core::{fmt::Display, str::FromStr};
use cozy_chess::{
    util::{display_uci_move, parse_uci_move},
    Board, Move, MoveParseError,
};
use crossbeam_channel::Sender;
use std::thread::JoinHandle;
use vampirc_uci::{UciInfoAttribute, UciMessage, UciMove, UciOptionConfig, UciTimeControl};

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
        hashfull: u16,
        pv: Vec<String>,
    },
}

pub enum UciToEngine {
    Uci,
    Debug(bool),
    IsReady,
    Register,
    Position(Board, Vec<History>),
    SetOption { name: String, value: Option<String> },
    UciNewGame,
    Stop,
    PonderHit,
    Quit,
    GoInfinite,
    GoMoveTime(Duration),
    GoGameTime(GameTime),
    Unknown(Option<String>),

    Eval,
    PrintBoard,
    PrintOptions,
    PlayMove(String),
    Help,
    RandomPosition,
    Sleep(u64),
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

                let msgs = vampirc_uci::parse_with_unknown(&incoming_data);

                for msg in msgs {
                    let report = match Self::handle_msg(msg) {
                        Ok(report) => report,
                        Err(err) => {
                            report_tx.send(EngineReport::Error(err)).unwrap();

                            continue;
                        }
                    };

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

    fn handle_msg(msg: UciMessage) -> Result<UciToEngine, String> {
        match msg {
            UciMessage::Uci => Ok(UciToEngine::Uci),

            UciMessage::Debug(debug) => Ok(UciToEngine::Debug(debug)),

            UciMessage::IsReady => Ok(UciToEngine::IsReady),

            UciMessage::Register {
                later: _,
                name: _,
                code: _,
            } => Ok(UciToEngine::Register),

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

                let mut board = Board::from_str(&fen).map_err(|err| err.to_string())?;
                let mut history = Vec::with_capacity(moves.len());

                for m in &moves {
                    board
                        .try_play(convert_move_from_uci(&board, m).map_err(|err| err.to_string())?)
                        .map_err(|err| format!("{m}: {err}"))?;

                    history.push(History {
                        hash: board.hash(),
                        is_reversible_move: board.halfmove_clock() != 0,
                    });
                }

                Ok(UciToEngine::Position(board, history))
            }

            UciMessage::SetOption { name, value } => Ok(UciToEngine::SetOption { name, value }),

            UciMessage::UciNewGame => Ok(UciToEngine::UciNewGame),

            UciMessage::Stop => Ok(UciToEngine::Stop),

            UciMessage::PonderHit => Ok(UciToEngine::PonderHit),

            UciMessage::Quit => Ok(UciToEngine::Quit),

            UciMessage::Go {
                time_control,
                search_control,
            } => time_control.map_or_else(
                || {
                    search_control.map_or_else(
                        || unreachable!(),
                        |search_control| {
                            Err(format!("search_control not supported: {search_control:?}"))
                        },
                    )
                },
                |time_control| match time_control {
                    UciTimeControl::Ponder => Err("ponder not supported".to_string()),
                    UciTimeControl::Infinite => Ok(UciToEngine::GoInfinite),
                    UciTimeControl::TimeLeft {
                        white_time,
                        black_time,
                        white_increment,
                        black_increment,
                        moves_to_go,
                    } => Ok(UciToEngine::GoGameTime(GameTime {
                        white_time: white_time.unwrap_or_default(),
                        black_time: black_time.unwrap_or_default(),
                        white_increment: white_increment.unwrap_or_default(),
                        black_increment: black_increment.unwrap_or_default(),
                        moves_to_go,
                    })),
                    UciTimeControl::MoveTime(movetime) => Ok(UciToEngine::GoMoveTime(movetime)),
                },
            ),

            UciMessage::Unknown(text, maybe_error) => {
                custom_command(&text, maybe_error.map(|e| e.to_string()))
            }

            UciMessage::Id { .. }
            | UciMessage::UciOk
            | UciMessage::ReadyOk
            | UciMessage::BestMove { .. }
            | UciMessage::CopyProtection(_)
            | UciMessage::Registration(_)
            | UciMessage::Option(_)
            | UciMessage::Info(_) => Err("unexpected message".to_string()),
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
                        println!(
                            "{}",
                            UciMessage::id_name(&format!(
                                "{} v{}",
                                env!("CARGO_PKG_NAME"),
                                env!("CARGO_PKG_VERSION")
                            ))
                        );
                        println!(
                            "{}",
                            UciMessage::id_author(
                                &env!("CARGO_PKG_AUTHORS")
                                    .split(':')
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            )
                        );

                        println!(
                            "{}",
                            UciMessage::Option(UciOptionConfig::Spin {
                                name: HashOption::name().to_owned(),
                                default: Some(HashOption::default()),
                                min: Some(HashOption::min()),
                                max: Some(HashOption::max()),
                            })
                        );

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
                        hashfull,
                        pv,
                    } => {
                        let score = if cp.abs() > EVAL_INFINITY - 256 {
                            let mate_in_plies = EVAL_INFINITY - cp.abs();
                            let sign = cp.signum();

                            let mate_in_moves = mate_in_plies / 2 + mate_in_plies % 2;

                            UciInfoAttribute::from_mate((mate_in_moves * sign).try_into().unwrap())
                        } else {
                            UciInfoAttribute::from_centipawns(cp.into())
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
                                UciInfoAttribute::HashFull(hashfull),
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

fn custom_command(text: &str, maybe_error: Option<String>) -> Result<UciToEngine, String> {
    let split_cmd = text.split_whitespace().collect::<Vec<_>>();

    match split_cmd.first() {
        Some(&"eval") => Ok(UciToEngine::Eval),
        Some(&"board") => Ok(UciToEngine::PrintBoard),
        Some(&"options") => Ok(UciToEngine::PrintOptions),
        Some(&"make") => {
            let mv = split_cmd
                .get(1)
                .copied()
                .ok_or_else(|| "no move provided".to_string())?;

            Ok(UciToEngine::PlayMove(mv.to_string()))
        }
        Some(&"help") => Ok(UciToEngine::Help),
        Some(&"random") => Ok(UciToEngine::RandomPosition),
        Some(&"sleep") => {
            let sleep_time = split_cmd
                .get(1)
                .copied()
                .ok_or_else(|| "no time provided".to_string())?;

            let sleep_time = sleep_time
                .parse::<u64>()
                .map_err(|err| format!("invalid time: {err}"))?;

            std::thread::sleep(core::time::Duration::from_millis(sleep_time));

            Ok(UciToEngine::Sleep(sleep_time))
        }

        _ => Err(maybe_error.unwrap_or_else(|| "unknown command".to_string())),
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
