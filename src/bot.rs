use crate::board::{Board, HEIGHT, WIDTH};
use crate::tetromino::{PieceShape, PieceShapes, WallKicks};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, mpsc};
use std::thread;

pub enum TetrominoType {
    I, J, L, O, S, Z, T
}


#[derive(Clone, Copy, PartialEq)]
pub enum TetrisAction {
    C, W, L, R, D, H, E
}

#[derive(Clone, Copy, PartialEq)]
pub enum CellState {
    Filled, Empty
}

pub struct BoardState {
    pub tetromino_row: usize,
    pub tetromino_col: usize,
    pub tetromino_rotation: usize,
    pub tetromino: TetrominoType,
    pub board: [[CellState; 10]; 20],
}

pub trait SinglePlayerBot {
    /// Create a new bot.
    fn new() -> Self;

    /// Gets an action to perform based on the board state. This should be called
    /// every frame.
    fn get_move(&mut self, board_state: &BoardState) -> TetrisAction;
}

pub struct SimpleBot {
    pub multithreaded: bool,
    workers: Vec<Worker>,
    shapes: PieceShapes,
    wall_kicks: WallKicks,
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
    fn new() -> Self {
        Self {
            multithreaded: true,
            workers: Vec::new(),
            shapes: serde_json::from_str(include_str!("../resources/tetrominoes.json")).expect("Invalid tetrominoes.json"),
            wall_kicks: serde_json::from_str(include_str!("../resources/wall_kicks.json")).expect("Invalid wall_kicks.json"),
        }
    }

    fn get_move(&mut self, state: &BoardState) -> TetrisAction {
        // Start workers once, when first needed.
        if self.multithreaded && self.workers.is_empty() {
            for _ in 0..4 {
                self.workers.push(Worker::new());
            }
        }

        let piece = match state.tetromino {
            TetrominoType::I => "i",
            TetrominoType::J => "j",
            TetrominoType::L => "l",
            TetrominoType::O => "o",
            TetrominoType::S => "s",
            TetrominoType::Z => "z",
            TetrominoType::T => "t",
        };
        let shape = &self.shapes[piece];
        let kicks = (&self.wall_kicks[&shape.wall_kick]);
        let kicks = Arc::new(kicks.clone());
        let shape = Arc::new(PieceShape {
            states: shape.states,
            wall_kick: shape.wall_kick.clone(),
        });

        let mut board = Board::new();
        for row in 0..HEIGHT {
            for col in 0..WIDTH {
                board.cells[row][col] = (state.board[row][col] == CellState::Filled);
            }
        }
        let board = Arc::new(board);

        let start = SearchPosition {
            row: state.tetromino_row as i32,
            col: state.tetromino_col as i32,
            rotation: state.tetromino_rotation,
            first_action: None,
        };
        let mut visited = HashSet::new();
        visited.insert((start.row, start.col, start.rotation));
        let mut current_depth = vec![start];
        let mut best_score = i32::MIN;
        let mut best_action = TetrisAction::H;

        while !current_depth.is_empty() {
            let mut worker_results = Vec::new();
            if self.multithreaded {
                let chunk_size = (current_depth.len() + 3) / 4;
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
            if landing_row + block[1] >= HEIGHT as i32 {
                topped_out = true;
                break;
            }
        }

        if !topped_out {
            let mut placed_board = Board { cells: board.cells };
            placed_board.place_piece(landing_row, col, blocks);
            let lines = placed_board.remove_full_rows();
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
                        let new_row = row + old[1] - new[1];
                        let new_col = col + old[0] - new[0];
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
