use crate::board::{Board, HEIGHT, WIDTH};
use crate::tetromino::{PieceShapes, WallKicks};
use std::collections::{HashSet, VecDeque};

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
    shapes: PieceShapes,
    wall_kicks: WallKicks,
}

impl SinglePlayerBot for SimpleBot {
    fn new() -> Self {
        Self {
            shapes: serde_json::from_str(include_str!("../resources/tetrominoes.json")).expect("Invalid tetrominoes.json"),
            wall_kicks: serde_json::from_str(include_str!("../resources/wall_kicks.json")).expect("Invalid wall_kicks.json"),
        }
    }

    fn get_move(&mut self, state: &BoardState) -> TetrisAction {
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
        let directions = ["o", "r", "t", "l"];

        let mut board = Board::new();
        for row in 0..HEIGHT {
            for col in 0..WIDTH {
                board.cells[row][col] = (state.board[row][col] == CellState::Filled);
            }
        }

        let start = (
            state.tetromino_row as i32,
            state.tetromino_col as i32,
            state.tetromino_rotation,
        );
        let mut queue = VecDeque::new();
        queue.push_back((start, None));
        let mut visited = HashSet::new();
        visited.insert(start);
        let mut best_score = i32::MIN;
        let mut best_action = TetrisAction::H;

        // Try moving and rotating before a hard drop.
        while let Some(((row, col, rotation), first_action)) = queue.pop_front() {
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
                let mut result = Board { cells: board.cells };
                result.place_piece(landing_row, col, blocks);
                let lines = result.remove_full_rows();
                let score = score_board(&result, lines);
                if score > best_score {
                    best_score = score;
                    best_action = match first_action {
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

                let next = (next_Row, next_Col, next_Rotation);
                if !board.can_place(next_Row, next_Col, &shape.states[next_Rotation]) {
                    continue;
                }
                if visited.contains(&next) {
                    continue;
                }
                visited.insert(next);

                let first_move = match first_action {
                    Some(previous_action) => previous_action,
                    None => action,
                };
                queue.push_back((next, Some(first_move)));
            }
        }

        best_action
    }
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
