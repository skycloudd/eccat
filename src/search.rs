use crate::{
    evaluate::{evaluate, Eval, EVAL_INFINITY},
    oracle::Oracle,
    see,
    tt::{Entry, Flag, TranspositionTable},
    uci::{convert_move_to_uci, GameTime},
    EngineReport,
};
use arrayvec::ArrayVec;
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

    SetHash(usize),
    ClearHash,
}

pub enum SearchToEngine {
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

#[derive(Debug, Default)]
pub struct Search {
    handle: Option<JoinHandle<()>>,
    control_tx: Option<Sender<EngineToSearch>>,
}

impl Search {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn init(
        &mut self,
        report_tx: Sender<EngineReport>,
        board: Arc<Mutex<Board>>,
        history: Arc<Mutex<Vec<History>>>,
        transposition_table: Arc<Mutex<TranspositionTable>>,
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
                    EngineToSearch::SetHash(size) => {
                        transposition_table.lock().unwrap().resize(size);
                        halt = true;
                    }
                    EngineToSearch::ClearHash => {
                        transposition_table.lock().unwrap().clear();
                        halt = true;
                    }
                }

                if !halt && !quit {
                    let mut refs = SearchRefs {
                        board: &mut board.lock().unwrap(),
                        control_rx: &control_rx,
                        report_tx: &report_tx,
                        search_mode: &search_mode.unwrap(),
                        search_state: &mut SearchState::default(),
                        history: &mut history.lock().unwrap(),
                        transposition_table: &mut transposition_table.lock().unwrap(),
                    };

                    let (best_move, terminate) = iterative_deepening(&mut refs);

                    let report = SearchToEngine::BestMove(
                        convert_move_to_uci(refs.board, best_move).to_string(),
                    );

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

    pub fn send(
        &self,
        cmd: EngineToSearch,
    ) -> Result<(), crossbeam_channel::SendError<EngineToSearch>> {
        if let Some(tx) = &self.control_tx {
            tx.send(cmd)?;
        }

        Ok(())
    }
}

fn iterative_deepening(refs: &mut SearchRefs) -> (Move, Option<SearchTerminate>) {
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
            || clock / 20,
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

    refs.transposition_table.clear();

    refs.search_state.start_time = Some(Instant::now());

    while depth <= 128 && !stop {
        refs.search_state.depth = depth;

        let eval = negamax(
            refs,
            &mut root_pv,
            depth,
            -EVAL_INFINITY,
            EVAL_INFINITY,
            true,
            NodeType::Root,
        );

        check_terminate(refs);

        if refs.search_state.terminate.is_none() {
            if !root_pv.is_empty() {
                best_move = root_pv.first().copied();
            }

            let elapsed = refs.search_state.start_time.unwrap().elapsed();

            #[allow(
                clippy::cast_precision_loss,
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss
            )]
            let nps = (refs.search_state.nodes as f64 / elapsed.as_secs_f64()) as u64;

            let report = SearchToEngine::Summary {
                depth,
                seldepth: refs.search_state.seldepth,
                time: Duration::from_std(elapsed).unwrap(),
                cp: eval,
                nodes: refs.search_state.nodes,
                nps,
                hashfull: refs.transposition_table.hashfull(),
                pv: convert_pv_to_strings(&root_pv, refs.board.clone()),
            };

            refs.report_tx.send(EngineReport::Search(report)).unwrap();

            depth += 1;
        }

        let is_time_up = match refs.search_mode {
            SearchMode::GameTime(_) => {
                // probably cant finish the next depth in time,
                // so if we're at 60% of the allocated time,
                // we stop the search
                refs.search_state.start_time.unwrap().elapsed()
                    >= refs.search_state.allocated_time.mul_f32(0.6)
            }
            _ => false,
        };

        if is_time_up || refs.search_state.terminate.is_some() {
            stop = true;
        }
    }

    (
        best_move.unwrap_or_else(|| first_legal_move(refs.board).unwrap()),
        refs.search_state.terminate,
    )
}

fn first_legal_move(board: &Board) -> Option<Move> {
    let mut first_move = None;

    board.generate_moves(|mvs| {
        first_move = mvs.into_iter().next();
        true
    });

    first_move
}

#[allow(clippy::cognitive_complexity, clippy::too_many_lines)]
fn negamax(
    refs: &mut SearchRefs,
    pv: &mut Vec<Move>,
    mut depth: u8,
    mut alpha: Eval,
    mut beta: Eval,
    nmp_allowed: bool,
    node_type: NodeType,
) -> Eval {
    debug_assert!(alpha < beta);

    if refs.search_state.nodes % 0x2000 == 0 {
        check_terminate(refs);
    }

    if refs.search_state.terminate.is_some() {
        return 0;
    }

    refs.search_state.nodes += 1;

    let is_check = !refs.board.checkers().is_empty();

    if is_check {
        depth += 1;
    }

    if depth == 0 {
        return quiescence(refs, pv, alpha, beta);
    }

    let mut tt_value = None;
    let mut tt_move = None;

    if let Some(data) = refs.transposition_table.probe(refs.board.hash()) {
        let tt_result = data.get(depth, refs.search_state.ply, alpha, beta);

        tt_value = tt_result.0;
        tt_move = tt_result.1;
    }

    if let Some(tt_value) = tt_value {
        if refs.search_state.ply > 0 {
            return tt_value;
        }
    }

    let static_eval = tt_value
        .and_then(|eval| {
            if (eval < EVAL_INFINITY - 256) && (eval > 256 - EVAL_INFINITY) {
                Some(eval)
            } else {
                None
            }
        })
        .unwrap_or_else(|| evaluate(refs.board));

    if !matches!(node_type, NodeType::Root | NodeType::Pv) {
        let margin = if depth <= 4 {
            Some(30 * i16::from(depth))
        } else {
            None
        };

        if let Some(margin) = margin {
            let eval = static_eval.saturating_sub(margin);

            if eval >= beta {
                return eval;
            }
        }
    }

    let mut moves: ArrayVec<cozy_chess::Move, MAX_MOVES> = generate_moves(refs.board, false);

    order_moves(refs, &mut moves, tt_move);

    let futile = [293, 620]
        .get(usize::from(depth))
        .map_or(false, |&margin| static_eval.saturating_add(margin) <= alpha);

    let is_game_over = moves.is_empty();

    let mut hash_flag = Flag::Alpha;
    let mut best_move = None;
    let mut best_score = -EVAL_INFINITY - 1;

    for (move_idx, legal) in moves.into_iter().enumerate() {
        let old_pos = make_move(refs, legal);

        let is_quiet = !is_capture(&old_pos, legal) && legal.promotion.is_none();
        let gives_check = refs.board.checkers().is_empty();

        if best_move.is_some() && futile && is_quiet && !is_check && !gives_check {
            unmake_move(refs, old_pos);
            continue;
        }

        let mut node_pv = Vec::new();

        let mut eval_score = 0;

        let reduction = if depth >= 3
            && move_idx >= 3
            && !is_check
            && legal.promotion.is_none()
            && refs.board.checkers().is_empty()
        {
            2
        } else {
            0
        };

        if !is_draw(refs) {
            if move_idx != 0 {
                eval_score = -negamax(
                    refs,
                    &mut node_pv,
                    (depth - 1).saturating_sub(reduction),
                    -alpha - 1,
                    -alpha,
                    nmp_allowed,
                    NodeType::Other,
                );

                if eval_score > alpha {
                    eval_score = -negamax(
                        refs,
                        &mut node_pv,
                        depth - 1,
                        -beta,
                        -alpha,
                        nmp_allowed,
                        node_type,
                    );
                }
            } else {
                eval_score = -negamax(
                    refs,
                    &mut node_pv,
                    depth - 1,
                    -beta,
                    -alpha,
                    nmp_allowed,
                    NodeType::Pv,
                );
            }
        }

        unmake_move(refs, old_pos);

        if eval_score > best_score {
            best_score = eval_score;
            best_move = Some(legal);
        }

        let mating_value = EVAL_INFINITY - Eval::from(refs.search_state.ply);

        if beta > mating_value {
            beta = mating_value;

            if alpha >= beta {
                return beta;
            }
        }

        let mating_value = Eval::from(refs.search_state.ply) - EVAL_INFINITY;

        if mating_value > alpha {
            alpha = mating_value;

            if beta <= alpha {
                return alpha;
            }
        }

        if eval_score >= beta {
            refs.transposition_table.insert(Entry::new(
                refs.board.hash(),
                depth,
                Flag::Beta,
                beta,
                best_move,
            ));

            if !is_capture(refs.board, legal) {
                store_killer_move(refs, legal);
            }

            return beta;
        }

        if eval_score > alpha {
            alpha = eval_score;

            hash_flag = Flag::Exact;

            pv.clear();
            pv.push(legal);
            pv.append(&mut node_pv);
        }
    }

    if is_game_over {
        if is_check {
            return -EVAL_INFINITY + Eval::from(refs.search_state.ply);
        }

        return 0;
    }

    refs.transposition_table.insert(Entry::new(
        refs.board.hash(),
        depth,
        hash_flag,
        alpha,
        best_move,
    ));

    alpha
}

fn quiescence(refs: &mut SearchRefs, pv: &mut Vec<Move>, mut alpha: Eval, beta: Eval) -> Eval {
    if refs.search_state.nodes % 0x2000 == 0 {
        check_terminate(refs);
    }

    if refs.search_state.terminate.is_some() {
        return 0;
    }

    refs.search_state.nodes += 1;

    let stand_pat = evaluate(refs.board);

    if stand_pat >= beta {
        return beta;
    }

    if stand_pat > alpha {
        alpha = stand_pat;
    }

    let mut moves: ArrayVec<cozy_chess::Move, MAX_MOVES> = generate_moves(refs.board, true);

    order_moves(refs, &mut moves, None);

    for legal in moves {
        let old_pos = make_move(refs, legal);

        let mut node_pv = Vec::new();

        let eval_score = -quiescence(refs, &mut node_pv, -beta, -alpha);

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

const MAX_MOVES: usize = 218;

#[must_use]
pub fn generate_moves(board: &Board, captures_only: bool) -> ArrayVec<Move, MAX_MOVES> {
    let mut moves = ArrayVec::new();

    board.generate_moves(|mvs| {
        if captures_only {
            moves.extend(
                mvs.into_iter()
                    .filter(|mv| is_capture(board, *mv) && see::see(board, *mv) >= 0),
            );
        } else {
            moves.extend(mvs);
        }

        false
    });

    moves
}

fn order_moves(refs: &SearchRefs, moves: &mut [Move], pv: Option<Move>) {
    pdqsort::sort_by(moves, |a, b| {
        let a_score = order_score(refs, *a, pv);
        let b_score = order_score(refs, *b, pv);

        b_score.cmp(&a_score)
    });
}

fn order_score(refs: &SearchRefs, mv: cozy_chess::Move, pv: Option<Move>) -> MoveScore {
    if let Some(pv) = pv {
        if mv == pv {
            return MoveScore::Pv;
        }
    }

    if matches!(mv.promotion, Some(piece) if piece != Piece::Queen) {
        return MoveScore::UnderPromotion;
    }

    if is_capture(refs.board, mv) {
        let see_eval = see::see(refs.board, mv);

        if see_eval >= 0 {
            return MoveScore::Capture(see_eval);
        }

        return MoveScore::LosingCapture(see_eval);
    }

    let ply = usize::from(refs.search_state.ply);

    for i in 0..2 {
        if refs.search_state.killer_moves[ply][i] == Some(mv) {
            return MoveScore::Killer;
        }
    }

    MoveScore::NonCapture
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum MoveScore {
    UnderPromotion,
    NonCapture,
    LosingCapture(i16),
    Killer,
    Capture(i16),
    Pv,
}

pub fn is_capture(board: &Board, legal: Move) -> bool {
    board.occupied().has(legal.to)
}

fn make_move(refs: &mut SearchRefs, legal: Move) -> Board {
    let old_pos = refs.board.clone();

    refs.board.play_unchecked(legal);

    refs.history.push(History {
        hash: refs.board.hash(),
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

            EngineToSearch::Start(_) | EngineToSearch::SetHash(_) | EngineToSearch::ClearHash => {}
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
        SearchMode::Depth(depth) => {
            if refs.search_state.depth > *depth {
                refs.search_state.terminate = Some(SearchTerminate::Stop);
            }
        }
    }
}

fn is_draw(refs: &mut SearchRefs) -> bool {
    Oracle::is_draw(refs.board) || is_threefold_repetition(refs) || is_fifty_move_rule(refs)
}

fn is_threefold_repetition(refs: &mut SearchRefs) -> bool {
    refs.history
        .iter()
        .rev()
        .take(refs.board.halfmove_clock() as usize + 1)
        .step_by(2)
        .filter(|entry| entry.hash == refs.board.hash())
        .count()
        >= 2
}

fn is_fifty_move_rule(refs: &mut SearchRefs) -> bool {
    refs.board.halfmove_clock() >= 100
}

fn store_killer_move(refs: &mut SearchRefs, mv: Move) {
    let ply = usize::from(refs.search_state.ply);

    let first_killer = refs.search_state.killer_moves[ply][0];

    if first_killer != Some(mv) {
        refs.search_state.killer_moves[ply][1] = first_killer;

        refs.search_state.killer_moves[ply][0] = Some(mv);
    }
}

#[derive(Debug)]
struct SearchRefs<'a> {
    board: &'a mut Board,
    control_rx: &'a Receiver<EngineToSearch>,
    report_tx: &'a Sender<EngineReport>,
    search_mode: &'a SearchMode,
    search_state: &'a mut SearchState,
    history: &'a mut Vec<History>,
    transposition_table: &'a mut TranspositionTable,
}

#[derive(Debug)]
pub struct History {
    pub hash: u64,
}

#[derive(Debug)]
pub enum SearchMode {
    Infinite,
    MoveTime(Duration),
    GameTime(GameTime),
    Depth(u8),
}

#[derive(Debug)]
struct SearchState {
    nodes: u64,
    ply: u8,
    depth: u8,
    seldepth: u8,
    terminate: Option<SearchTerminate>,
    start_time: Option<Instant>,
    allocated_time: core::time::Duration,
    killer_moves: [[Option<Move>; 2]; 128],
}

impl Default for SearchState {
    fn default() -> Self {
        Self {
            nodes: Default::default(),
            ply: Default::default(),
            depth: Default::default(),
            seldepth: Default::default(),
            terminate: Option::default(),
            start_time: Option::default(),
            allocated_time: core::time::Duration::default(),
            killer_moves: [[None; 2]; 128],
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum SearchTerminate {
    Stop,
    Quit,
}

#[derive(Clone, Copy, Debug)]
pub enum NodeType {
    Root,
    Pv,
    Other,
}

fn convert_pv_to_strings(pv: &[Move], mut board: Board) -> Vec<String> {
    pv.iter()
        .map(|m| {
            let str = convert_move_to_uci(&board, *m).to_string();
            board.play(*m);
            str
        })
        .collect()
}
