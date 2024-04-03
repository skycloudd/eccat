use crate::{
    evaluate::{evaluate, Eval},
    uci::GameTime,
    EngineReport,
};
use chrono::Duration;
use cozy_chess::{Board, Color, Move, Piece};
use crossbeam_channel::{Receiver, Sender};
use std::{
    sync::{Arc, Mutex},
    thread::JoinHandle,
    time::Instant,
};

pub enum EngineToSearch {
    Start(SearchMode),
    Stop,
    Quit,
}

pub enum SearchToEngine {
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

#[derive(Debug, Default)]
pub struct Search {
    handle: Option<JoinHandle<()>>,
    control_tx: Option<Sender<EngineToSearch>>,
}

impl Search {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn init(
        &mut self,
        report_tx: Sender<EngineReport>,
        board: Arc<Mutex<Board>>,
        history: Arc<Mutex<Vec<History>>>,
    ) {
        let (control_tx, control_rx) = crossbeam_channel::unbounded();

        let handle = std::thread::spawn(move || {
            let mut quit = false;
            let mut halt = true;

            while !quit {
                let cmd = control_rx.recv().unwrap();

                let mut search_mode = None;

                match cmd {
                    EngineToSearch::Start(sm) => {
                        search_mode = Some(sm);
                        halt = false;
                    }
                    EngineToSearch::Stop => halt = true,
                    EngineToSearch::Quit => quit = true,
                }

                if !halt && !quit {
                    let mut refs = SearchRefs {
                        board: &mut board.lock().unwrap(),
                        control_rx: &control_rx,
                        report_tx: &report_tx,
                        search_mode: &search_mode.unwrap(),
                        search_state: &mut SearchState::default(),
                        history: &mut history.lock().unwrap(),
                    };

                    let (best_move, terminate) = Self::iterative_deepening(&mut refs);

                    let report = SearchToEngine::BestMove(best_move.unwrap());

                    report_tx.send(EngineReport::Search(report)).unwrap();

                    if let Some(terminate) = terminate {
                        match terminate {
                            SearchTerminate::Stop => {
                                halt = true;
                            }
                            SearchTerminate::Quit => {
                                halt = true;
                                quit = true;
                            }
                        }
                    }
                }
            }
        });

        self.handle = Some(handle);
        self.control_tx = Some(control_tx);
    }

    pub fn send(&self, cmd: EngineToSearch) {
        if let Some(tx) = &self.control_tx {
            tx.send(cmd).unwrap();
        }
    }

    fn iterative_deepening(refs: &mut SearchRefs) -> (Option<Move>, Option<SearchTerminate>) {
        let mut best_move = None;
        let mut root_pv = Vec::new();
        let mut depth = 1;
        let mut stop = false;

        if let SearchMode::GameTime(gametime) = &refs.search_mode {
            let (clock, increment) = match refs.board.side_to_move() {
                Color::White => (gametime.white_time, gametime.white_increment),
                Color::Black => (gametime.black_time, gametime.black_increment),
            };

            let time = gametime.moves_to_go.map_or_else(
                || clock / 30,
                |mtg| {
                    if mtg == 0 {
                        clock
                    } else {
                        clock / i32::from(mtg)
                    }
                },
            );

            let time_slice = time + increment - Duration::milliseconds(100);

            refs.search_state.allocated_time = time_slice.to_std().unwrap_or_default();
        }

        refs.search_state.start_time = Some(Instant::now());

        while depth <= 128 && !stop {
            refs.search_state.depth = depth;

            let eval = Self::negamax(refs, &mut root_pv, depth, -Eval::INFINITY, Eval::INFINITY);

            check_terminate(refs);

            if refs.search_state.terminate.is_none() {
                if !root_pv.is_empty() {
                    best_move = root_pv.first().copied();
                }

                let elapsed = refs.search_state.start_time.unwrap().elapsed();

                let report = SearchToEngine::Summary {
                    depth,
                    seldepth: refs.search_state.seldepth,
                    time: Duration::from_std(elapsed).unwrap(),
                    cp: eval,
                    nodes: refs.search_state.nodes,
                    #[allow(
                        clippy::cast_precision_loss,
                        clippy::cast_possible_truncation,
                        clippy::cast_sign_loss
                    )]
                    nps: (refs.search_state.nodes as f64 / elapsed.as_secs_f64()) as u64,
                    pv: root_pv.clone(),
                };

                refs.report_tx.send(EngineReport::Search(report)).unwrap();

                depth += 1;
            }

            let is_time_up = match refs.search_mode {
                SearchMode::GameTime(_) => {
                    refs.search_state.start_time.unwrap().elapsed()
                        >= refs.search_state.allocated_time
                }
                _ => false,
            };

            if is_time_up || refs.search_state.terminate.is_some() {
                stop = true;
            }
        }

        (best_move, refs.search_state.terminate)
    }

    fn negamax(
        refs: &mut SearchRefs,
        pv: &mut Vec<Move>,
        mut depth: u8,
        mut alpha: Eval,
        beta: Eval,
    ) -> Eval {
        if refs.search_state.nodes % 0x1000 == 0 {
            check_terminate(refs);
        }

        if refs.search_state.terminate.is_some() {
            return Eval(0);
        }

        if refs.search_state.ply > 128 {
            return evaluate(refs.board);
        }

        refs.search_state.nodes += 1;

        let is_check = !refs.board.checkers().is_empty();

        if is_check {
            depth += 1;
        }

        if depth == 0 {
            return Self::quiescence(refs, pv, alpha, beta);
        }

        let moves = Self::generate_moves(refs.board, false);

        let is_game_over = moves.is_empty();

        for legal in moves {
            let old_pos = make_move(refs, legal);

            let mut node_pv = Vec::new();

            let eval_score = if is_draw(refs) {
                Eval(0)
            } else {
                -Self::negamax(refs, &mut node_pv, depth - 1, -beta, -alpha)
            };

            unmake_move(refs, old_pos);

            if eval_score >= beta {
                return beta;
            }

            if eval_score > alpha {
                alpha = eval_score;

                pv.clear();
                pv.push(legal);
                pv.append(&mut node_pv);
            }
        }

        if is_game_over {
            if is_check {
                return Eval(-Eval::INFINITY.0 + i16::from(refs.search_state.ply));
            }

            return Eval(0);
        }

        alpha
    }

    fn quiescence(refs: &mut SearchRefs, pv: &mut Vec<Move>, mut alpha: Eval, beta: Eval) -> Eval {
        if refs.search_state.nodes % 0x1000 == 0 {
            check_terminate(refs);
        }

        if refs.search_state.terminate.is_some() {
            return Eval(0);
        }

        if refs.search_state.ply > 128 {
            return evaluate(refs.board);
        }

        refs.search_state.nodes += 1;

        let stand_pat = evaluate(refs.board);

        if stand_pat >= beta {
            return beta;
        }

        if stand_pat > alpha {
            alpha = stand_pat;
        }

        let moves = Self::generate_moves(refs.board, true);

        for legal in moves {
            let old_pos = make_move(refs, legal);

            let mut node_pv = Vec::new();

            let eval_score = -Self::quiescence(refs, &mut node_pv, -beta, -alpha);

            unmake_move(refs, old_pos);

            if eval_score >= beta {
                return beta;
            }

            if eval_score > alpha {
                alpha = eval_score;

                pv.clear();
                pv.push(legal);
                pv.append(&mut node_pv);
            }
        }

        alpha
    }

    fn generate_moves(board: &Board, captures_only: bool) -> Vec<Move> {
        let mut moves = Vec::new();

        board.generate_moves(|mvs| {
            if captures_only {
                moves.extend(mvs.into_iter().filter(|mv| board.piece_on(mv.to).is_some()));
            } else {
                moves.extend(mvs);
            }

            false
        });

        moves
    }
}

fn make_move(refs: &mut SearchRefs, legal: Move) -> Board {
    let old_pos = refs.board.clone();

    refs.board.play(legal);

    refs.history.push(History {
        hash: refs.board.hash(),
        is_reversible_move: refs.board.halfmove_clock() != 0,
    });

    refs.search_state.ply += 1;

    if refs.search_state.ply > refs.search_state.seldepth {
        refs.search_state.seldepth = refs.search_state.ply;
    }

    old_pos
}

fn unmake_move(refs: &mut SearchRefs, old_pos: Board) {
    refs.search_state.ply -= 1;

    refs.history.pop();

    *refs.board = old_pos;
}

fn check_terminate(refs: &mut SearchRefs) {
    if let Ok(cmd) = refs.control_rx.try_recv() {
        match cmd {
            EngineToSearch::Stop => refs.search_state.terminate = Some(SearchTerminate::Stop),
            EngineToSearch::Quit => refs.search_state.terminate = Some(SearchTerminate::Quit),

            EngineToSearch::Start(_) => {}
        }
    }

    match refs.search_mode {
        SearchMode::Infinite => {}
        SearchMode::MoveTime(movetime) => {
            if refs.search_state.start_time.unwrap().elapsed().as_millis()
                > movetime.num_milliseconds().try_into().unwrap()
            {
                refs.search_state.terminate = Some(SearchTerminate::Stop);
            }
        }
        SearchMode::GameTime(_) => {
            if refs.search_state.start_time.unwrap().elapsed() > refs.search_state.allocated_time {
                refs.search_state.terminate = Some(SearchTerminate::Stop);
            }
        }
    }
}

fn is_draw(refs: &mut SearchRefs) -> bool {
    is_insufficient_material(refs) || is_threefold_repetition(refs) || is_fifty_move_rule(refs)
}

fn is_threefold_repetition(refs: &mut SearchRefs) -> bool {
    let mut count = 0;

    for i in 0..refs.history.len() {
        if refs.history[i].hash == refs.board.hash() {
            count += 1;
        }
    }

    count >= 3
}

fn is_fifty_move_rule(refs: &mut SearchRefs) -> bool {
    let mut count = 0;

    for i in 0..refs.history.len() {
        if refs.history[i].is_reversible_move {
            count += 1;
        } else {
            count = 0;
        }

        if count >= 100 {
            return true;
        }
    }

    false
}

fn is_insufficient_material(refs: &mut SearchRefs) -> bool {
    let white = refs.board.colors(Color::White);
    let black = refs.board.colors(Color::Black);

    let white_queens = refs.board.pieces(Piece::Queen) & white;
    let black_queens = refs.board.pieces(Piece::Queen) & black;

    if !white_queens.is_empty() || !black_queens.is_empty() {
        return false;
    }

    let white_rooks = refs.board.pieces(Piece::Rook) & white;
    let black_rooks = refs.board.pieces(Piece::Rook) & black;

    if !white_rooks.is_empty() || !black_rooks.is_empty() {
        return false;
    }

    let white_bishops = refs.board.pieces(Piece::Bishop) & white;
    let black_bishops = refs.board.pieces(Piece::Bishop) & black;

    if !white_bishops.is_empty() || !black_bishops.is_empty() {
        return false;
    }

    let white_knights = refs.board.pieces(Piece::Knight) & white;
    let black_knights = refs.board.pieces(Piece::Knight) & black;

    if !white_knights.is_empty() || !black_knights.is_empty() {
        return false;
    }

    let white_pawns = refs.board.pieces(Piece::Pawn) & white;
    let black_pawns = refs.board.pieces(Piece::Pawn) & black;

    if !white_pawns.is_empty() || !black_pawns.is_empty() {
        return false;
    }

    true
}

#[derive(Debug)]
struct SearchRefs<'a> {
    board: &'a mut Board,
    control_rx: &'a Receiver<EngineToSearch>,
    report_tx: &'a Sender<EngineReport>,
    search_mode: &'a SearchMode,
    search_state: &'a mut SearchState,
    history: &'a mut Vec<History>,
}

#[derive(Debug)]
pub struct History {
    pub hash: u64,
    pub is_reversible_move: bool,
}

#[derive(Debug)]
pub enum SearchMode {
    Infinite,
    MoveTime(Duration),
    GameTime(GameTime),
}

#[derive(Debug, Default)]
struct SearchState {
    nodes: u64,
    ply: u8,
    depth: u8,
    seldepth: u8,
    terminate: Option<SearchTerminate>,
    start_time: Option<Instant>,
    allocated_time: core::time::Duration,
}

#[derive(Clone, Copy, Debug)]
enum SearchTerminate {
    Stop,
    Quit,
}
