use crate::board::{Board, HEIGHT, WIDTH};
use crate::game::Game;
use crate::tetromino::PieceShape;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, mpsc};
use std::thread;

pub struct GarbageLine {
    pub num_rows_to_create: usize,
    pub ticks_till_applied: usize,
}

pub trait MultiPlayerBot {
    fn new(config: &Path) -> Self;
    fn get_move(&self) -> TetrisAction;
    fn update_with(&mut self, p_actions: &[TetrisAction]);
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TetrisAction {
    C, W, L, R, D, H, E
}

impl TetrisAction {
    pub fn as_char(self) -> char {
        match self {
            Self::C => 'C',
            Self::W => 'W',
            Self::L => 'L',
            Self::R => 'R',
            Self::D => 'D',
            Self::H => 'H',
            Self::E => 'E',
        }
    }
}

pub trait SinglePlayerBot {
    fn new(config: &Path) -> Self;
    fn get_move(&self) -> TetrisAction;
    fn update_with(&mut self, action: TetrisAction);
}

pub struct SimpleBot {
    workers: Vec<Worker>,
    pub game: Game,
}

pub struct VersusBot {
    player: SimpleBot,
    opponent: Game,
    hard_drop_only: bool,
}

impl VersusBot {
    pub fn new_bad(config: &Path) -> Self {
        Self {
            player: SimpleBot::with_threads(config, false),
            opponent: Game::load(config),
            hard_drop_only: true,
        }
    }

    pub fn games(&self) -> (&Game, &Game) {
        (&self.player.game, &self.opponent)
    }
}

impl MultiPlayerBot for VersusBot {
    fn new(config: &Path) -> Self {
        Self {
            player: SimpleBot::new(config),
            opponent: Game::load(config),
            hard_drop_only: false,
        }
    }

    fn get_move(&self) -> TetrisAction {
        if self.hard_drop_only {
            return TetrisAction::H;
        }
        self.player.get_move()
    }

    fn update_with(&mut self, p_actions: &[TetrisAction]) {
        Game::multiplayer_tick(
            &mut self.player.game,
            &mut self.opponent,
            p_actions[0].as_char(),
            p_actions[1].as_char(),
        );
    }
}

#[derive(Clone)]
struct SearchPosition {
    row: i32,
    col: i32,
    rotation: usize,
    first_action: Option<TetrisAction>,
}

struct WorkerResult {
    neighbors: Vec<SearchPosition>,
    best_score: i32,
    best_action: TetrisAction,
}

struct SearchJob {
    positions: Vec<SearchPosition>,
    board: Arc<Board>,
    shape: Arc<PieceShape>,
    kicks: Arc<HashMap<String, Vec<[i32; 2]>>>,
}

struct Worker {
    jobs: mpsc::Sender<Option<SearchJob>>,
    results: mpsc::Receiver<WorkerResult>,
    handle: thread::JoinHandle<()>,
}

impl Worker {
    fn new() -> Self {
        let (job_sender, job_receiver) = mpsc::channel::<Option<SearchJob>>();
        let (result_sender, result_receiver) = mpsc::channel();

        let handle = thread::spawn(move || {
            while let Ok(message) = job_receiver.recv() {
                let job = match message {
                    Some(job) => job,
                    None => break,
                };
                let result = search_chunk(&job.positions, &job.board, &job.shape, &job.kicks);
                if result_sender.send(result).is_err() {
                    break;
                }
            }
        });

        Self {
            jobs: job_sender,
            results: result_receiver,
            handle,
        }
    }
}

impl Drop for SimpleBot {
    fn drop(&mut self) {
        for worker in &self.workers {
            let _ = worker.jobs.send(None);
        }
        for worker in self.workers.drain(..) {
            let _ = worker.handle.join();
        }
    }
}

impl SinglePlayerBot for SimpleBot {
    fn new(config: &Path) -> Self {
        Self::with_threads(config, true)
    }

    fn get_move(&self) -> TetrisAction {
        if self.game.game_over {
            return TetrisAction::E;
        }
        self.search()
    }

    fn update_with(&mut self, action: TetrisAction) {
        self.game.tick(action.as_char());
    }
}

impl SimpleBot {
    pub fn with_threads(config: &Path, multithreaded: bool) -> Self {
        let game = Game::load(config);
        let mut workers = Vec::new();
        if multithreaded {
            for _ in 0..4 {
                workers.push(Worker::new());
            }
        }
        Self { workers, game }
    }

    fn search(&self) -> TetrisAction {
        let piece = &self.game.current_piece;
        let shape = &self.game.shapes[&piece.piece_type];
        let kicks = Arc::new(self.game.wall_kicks[&shape.wall_kick].clone());
        let shape = Arc::new(PieceShape {
            states: shape.states,
            wall_kick: shape.wall_kick.clone(),
        });

        let board = Arc::new(Board { cells: self.game.board.cells });
        let start = SearchPosition {
            row: piece.row,
            col: piece.col,
            rotation: piece.rotation,
            first_action: None,
        };
        let mut visited = HashSet::new();
        visited.insert((start.row, start.col, start.rotation));
        let mut current_depth = vec![start];
        let mut best_score = i32::MIN;
        let mut best_action = TetrisAction::H;

        while !current_depth.is_empty() {
            let mut worker_results = Vec::new();
            if !self.workers.is_empty() {
                let chunk_size = ((current_depth.len() + 3) / 4);
                let mut jobs_sent = 0;

                for chunk in current_depth.chunks(chunk_size) {
                    let job = SearchJob {
                        positions: chunk.to_vec(),
                        board: Arc::clone(&board),
                        shape: Arc::clone(&shape),
                        kicks: Arc::clone(&kicks),
                    };
                    self.workers[jobs_sent].jobs.send(Some(job)).expect("Search worker stopped");
                    jobs_sent += 1;
                }

                for i in 0..jobs_sent {
                    let result = self.workers[i].results.recv().expect("Search worker failed");
                    worker_results.push(result);
                }
            } else {
                worker_results.push(search_chunk(&current_depth, &board, &shape, &kicks));
            }

            // Finish this depth before starting the next one.
            let mut next_depth = Vec::new();
            for result in worker_results {
                if result.best_score > best_score {
                    best_score = result.best_score;
                    best_action = result.best_action;
                }

                for neighbor in result.neighbors {
                    let position = (neighbor.row, neighbor.col, neighbor.rotation);
                    if !visited.contains(&position) {
                        visited.insert(position);
                        next_depth.push(neighbor);
                    }
                }
            }
            current_depth = next_depth;
        }

        best_action
    }
}

fn search_chunk(
    positions: &[SearchPosition],
    board: &Board,
    shape: &PieceShape,
    kicks: &HashMap<String, Vec<[i32; 2]>>,
) -> WorkerResult {
    let directions = ["o", "r", "t", "l"];
    let mut result = WorkerResult {
        neighbors: Vec::new(),
        best_score: i32::MIN,
        best_action: TetrisAction::H,
    };

    for position in positions {
        let row = position.row;
        let col = position.col;

        let rotation = position.rotation;
        let first_action = position.first_action;
        let blocks = &shape.states[rotation];
        if !board.can_place(row, col, blocks) {
            continue;
        }

        let mut landing_row = row;
        while board.can_place(landing_row - 1, col, blocks) {
            landing_row -= 1;

        }

        // A piece above the visible board would lose.
        let mut topped_out = false;
        for block in blocks {
            if landing_row + block[1] >= (HEIGHT as i32) {
                topped_out = true;
                break;
            }

        }

        if !topped_out {
            let mut placed_board = Board { cells: board.cells };
            placed_board.place_piece(landing_row, col, blocks);
            let lines =( placed_board.remove_full_rows());
            let score = score_board(&placed_board, lines);

            if score > result.best_score {
                result.best_score = score;
                result.best_action = match first_action {
                    Some(action) => action,
                    None => TetrisAction::H,
                };
            }
        }

        for action in [
            TetrisAction::L,
            TetrisAction::R,
            TetrisAction::C,
            TetrisAction::W,
        ] {
            let mut next_Row = row;
            let mut next_Col = col;
            let mut next_Rotation = rotation;
            match action {
                TetrisAction::L => next_Col -= 1,
                TetrisAction::R => next_Col += 1,
                TetrisAction::C | TetrisAction::W => {
                    let mut turn = 3;
                    if action == TetrisAction::C {
                        turn = 1;
                    }
                    let new_rotation = (rotation + turn) % 4;
                    let old_offsets = &kicks[directions[rotation]];
                    let new_offsets = &kicks[directions[new_rotation]];
                    for i in 0..old_offsets.len() {
                        let old = old_offsets[i];
                        let new = new_offsets[i];
                        let new_row = (row + old[1] - new[1]);
                        let new_col = (col + old[0] - new[0]);
                        if board.can_place(new_row, new_col, &shape.states[new_rotation]) {
                            next_Row = new_row;
                            next_Col = new_col;
                            next_Rotation = new_rotation;
                            break;
                        }
                    }
                }
                _ => continue,
            }

            if !board.can_place(next_Row, next_Col, &shape.states[next_Rotation]) {
                continue;
            }
            let first_move = match first_action {
                Some(previous_action) => previous_action,
                None => action,
            };
            result.neighbors.push(SearchPosition {
                row: next_Row,
                col: next_Col,
                rotation: next_Rotation,
                first_action: Some(first_move),
            });
        }
    }

    result
}

fn score_board(board: &Board, lines: usize) -> i32 {
    let mut heights = [0; WIDTH];
    let mut holes = 0;

    for col in 0..WIDTH {
        let mut block_above = false;
        for row in (0..HEIGHT).rev() {
            if board.cells[row][col] {
                if !block_above {
                    heights[col] = (row + 1) as i32;
                }
                block_above = true;
            } else if block_above {
                holes += 1;
            }
        }
    }

    let mut total_height = 0;
    for height in heights {
        total_height += height;
    }
    let mut bumpiness = 0;
    for col in 0..WIDTH - 1 {
        bumpiness += (heights[col] - heights[col + 1]).abs();
    }

    // Prefer low, even boards with few holes.
    8 * lines as i32 - 5 * total_height - 20 * holes - 3 * bumpiness
}
