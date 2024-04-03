use crate::{evaluate::Eval, EngineReport};
use chrono::Duration;
use cozy_chess::{File, Move, Piece, Rank, Square};
use crossbeam_channel::Sender;
use std::thread::JoinHandle;
use vampirc_uci::{UciInfoAttribute, UciMessage, UciMove, UciPiece, UciSquare, UciTimeControl};

pub enum EngineToUci {
    Identify,
    Ready,
    Quit,
    BestMove(Move),
    Summary {
        depth: u8,
        seldepth: u8,
        time: Duration,
        cp: Eval,
        nodes: u64,
        nps: u64,
        pv: Vec<Move>,
    },
}

pub enum UciToEngine {
    Uci,
    Debug(bool),
    IsReady,
    Register,
    Position(String, Vec<Move>),
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
                    let report = match msg {
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
                                String::from(
                                    "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
                                )
                            } else {
                                fen.unwrap().to_string()
                            };

                            let moves = moves.into_iter().map(convert_move).collect();

                            UciToEngine::Position(fen, moves)
                        }

                        UciMessage::SetOption { name: _, value: _ } => UciToEngine::SetOption,

                        UciMessage::UciNewGame => UciToEngine::UciNewGame,

                        UciMessage::Stop => UciToEngine::Stop,

                        UciMessage::PonderHit => UciToEngine::PonderHit,

                        UciMessage::Quit => {
                            quit = true;

                            UciToEngine::Quit
                        }

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
                                UciTimeControl::MoveTime(movetime) => {
                                    UciToEngine::GoMoveTime(movetime)
                                }
                            },
                        ),

                        _ => UciToEngine::Unknown,
                    };

                    report_tx.send(EngineReport::Uci(report)).unwrap();
                }

                incoming_data.clear();
            }
        });

        self.report_handle = Some(report_handle);
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
                        println!("{}", UciMessage::best_move(convert_move_back(bestmove)));
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
                        let (cp, mate) = if cp.abs() > *Eval::INFINITY / 2 {
                            let mate_in_plies = *Eval::INFINITY - cp.abs();
                            let sign = cp.signum();

                            let mate_in_moves = mate_in_plies / 2 + mate_in_plies % 2;

                            (None, Some(mate_in_moves * sign))
                        } else {
                            (Some(cp), None)
                        };

                        println!(
                            "{}",
                            UciMessage::Info(vec![
                                UciInfoAttribute::Depth(depth),
                                UciInfoAttribute::SelDepth(seldepth),
                                UciInfoAttribute::Time(time),
                                UciInfoAttribute::Score {
                                    cp: cp.map(|cp| cp.0.into()),
                                    mate: mate.map(|mate| mate.try_into().unwrap()),
                                    lower_bound: None,
                                    upper_bound: None
                                },
                                UciInfoAttribute::Nodes(nodes),
                                UciInfoAttribute::Nps(nps),
                                UciInfoAttribute::Pv(
                                    pv.into_iter().map(convert_move_back).collect()
                                )
                            ])
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

fn convert_move(m: UciMove) -> Move {
    let from = convert_square(m.from);
    let to = convert_square(m.to);

    let promotion = m.promotion.map(|p| match p {
        UciPiece::Pawn => Piece::Pawn,
        UciPiece::Knight => Piece::Knight,
        UciPiece::Bishop => Piece::Bishop,
        UciPiece::Rook => Piece::Rook,
        UciPiece::Queen => Piece::Queen,
        UciPiece::King => Piece::King,
    });

    Move {
        from,
        to,
        promotion,
    }
}

fn convert_square(s: UciSquare) -> Square {
    let file = File::index(s.file as usize);
    let rank = Rank::index(s.rank as usize);

    Square::new(file, rank)
}

fn convert_move_back(m: Move) -> UciMove {
    let from = convert_square_back(m.from);
    let to = convert_square_back(m.to);

    let promotion = m.promotion.map(|p| match p {
        Piece::Pawn => UciPiece::Pawn,
        Piece::Knight => UciPiece::Knight,
        Piece::Bishop => UciPiece::Bishop,
        Piece::Rook => UciPiece::Rook,
        Piece::Queen => UciPiece::Queen,
        Piece::King => UciPiece::King,
    });

    UciMove {
        from,
        to,
        promotion,
    }
}

const fn convert_square_back(s: Square) -> UciSquare {
    UciSquare {
        file: match s.file() {
            File::A => 'a',
            File::B => 'b',
            File::C => 'c',
            File::D => 'd',
            File::E => 'e',
            File::F => 'f',
            File::G => 'g',
            File::H => 'h',
        },
        rank: match s.rank() {
            Rank::First => 1,
            Rank::Second => 2,
            Rank::Third => 3,
            Rank::Fourth => 4,
            Rank::Fifth => 5,
            Rank::Sixth => 6,
            Rank::Seventh => 7,
            Rank::Eighth => 8,
        },
    }
}
