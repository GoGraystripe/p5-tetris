use crate::board::{Board, HEIGHT};
use crate::tetromino::{PieceBag, PieceShapes, Tetromino, WallKicks};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub gravity: u32,
    pub gravity_decrease: u32,
    pub lock_delay: u32,
    pub random_seed: [u8; 16],
}

pub struct Game {
    pub board: Board,
    pub current_piece: Tetromino,
    pub shapes: PieceShapes,
    pub wall_kicks: WallKicks,
    pub piece_bag: PieceBag,
    pub gravity: u32,
    pub gravity_decrease: u32,
    pub lock_delay: u32,
    pub gravity_timer: u32,
    pub lock_timer: u32,
    pub game_over: bool,
}

impl Game {
    pub fn new(config: Config, shapes: PieceShapes, wall_kicks: WallKicks) -> Self {
        let mut piece_bag = PieceBag::new(config.random_seed);
        let first_piece = Tetromino::new(piece_bag.next_piece());

        Self {
            board: Board::new(),
            current_piece: first_piece,
            shapes,
            wall_kicks,
            piece_bag,
            gravity: config.gravity,
            gravity_decrease: config.gravity_decrease,
            lock_delay: config.lock_delay,
            gravity_timer: 0,
            lock_timer: 0,
            game_over: false,
        }
    }

    fn can_move(&self, row: i32, col: i32, rotation: usize) -> bool {
        let shape = &self.shapes[&self.current_piece.piece_type];
        let blocks = &shape.states[rotation];

        self.board.can_place(row, col, blocks)
    }

    fn move_left(&mut self) {
        let row = self.current_piece.row;
        let col = self.current_piece.col - 1;
        let rotation = self.current_piece.rotation;

        if self.can_move(row, col, rotation) {
            self.current_piece.col = col;
        }
    }

    fn move_right(&mut self) {
        let row = self.current_piece.row;
        let col = self.current_piece.col + 1;
        let rotation = self.current_piece.rotation;

        if self.can_move(row, col, rotation) {
            self.current_piece.col = col;
        }
    }

    fn move_down(&mut self) -> bool {
        let row = self.current_piece.row - 1;
        let col = self.current_piece.col;
        let rotation = self.current_piece.rotation;

        if self.can_move(row, col, rotation) {
            self.current_piece.row = row;
            true
        } else {
            false
        }
    }

    fn hard_drop(&mut self) {
        while self.move_down() {}

        self.lock_piece();
    }

    // Try each wall kick until the rotation fits
    fn rotate(&mut self, clockwise: bool) {
        let old_rotation = self.current_piece.rotation;

        let new_rotation = if clockwise {
            (old_rotation + 1) % 4
        } else {
            (old_rotation + 3) % 4
        };

        let shape = &self.shapes[&self.current_piece.piece_type];
        let kick_type = &shape.wall_kick;

        let directions = ["o", "r", "t", "l"];
        let old_offsets = &self.wall_kicks[kick_type][directions[old_rotation]];
        let new_offsets = &self.wall_kicks[kick_type][directions[new_rotation]];

        for i in 0..old_offsets.len() {
            // wall kicks use the difference between rotation offsets
            let col = self.current_piece.col + old_offsets[i][0] - new_offsets[i][0];
            let row = self.current_piece.row + old_offsets[i][1] - new_offsets[i][1];

            if self.can_move(row, col, new_rotation) {
                self.current_piece.row = row;
                self.current_piece.col = col;
                self.current_piece.rotation = new_rotation;
                return;
            }
        }
    }

    // check for blocks above the playable area
    fn is_topped_out(&self) -> bool {
        let blocks = self.current_piece.blocks(&self.shapes);

        for block in blocks {
            let row = self.current_piece.row + block[1];

            if row >= HEIGHT as i32 {
                return true;
            }
        }

        false
    }

    fn lock_piece(&mut self) {
        let topped_out = self.is_topped_out();
        let blocks = self.current_piece.blocks(&self.shapes);

        self.board
            .place_piece(self.current_piece.row, self.current_piece.col, blocks);

        if topped_out {
            self.game_over = true;
            return;
        }

        self.board.remove_full_rows();

        self.current_piece = Tetromino::new(self.piece_bag.next_piece());
        self.gravity_timer = 0;
        self.lock_timer = 0;

        let row = self.current_piece.row;
        let col = self.current_piece.col;
        let rotation = self.current_piece.rotation;

        if !self.can_move(row, col, rotation) {
            self.game_over = true;
        }
    }

    pub fn tick(&mut self, action: char) {
        if self.game_over {
            return;
        }

        match action {
            'L' => self.move_left(),
            'R' => self.move_right(),
            'C' => self.rotate(true),
            'W' => self.rotate(false),
            'H' => {
                self.hard_drop();
                return;
            }
            'D' | 'E' => {}
            _ => {}
        }

        self.gravity_timer += 1;

        // Soft drop temporarily reduces the gravity interval, but does not reset the gravity timer
        let gravity_interval = if action == 'D' {
            self.gravity.saturating_sub(self.gravity_decrease).max(1)
        } else {
            self.gravity.max(1)
        };

        if self.gravity_timer >= gravity_interval {
            self.move_down();
            self.gravity_timer = 0;
        }

        let row = self.current_piece.row - 1;
        let col = self.current_piece.col;
        let rotation = self.current_piece.rotation;

        if !self.can_move(row, col, rotation) {
            self.lock_timer += 1;

            if self.lock_timer >= self.lock_delay {
                self.lock_piece();
            }
        } else {
            self.lock_timer = 0;
        }
    }

    pub fn render(&self) -> String {
        if self.game_over {
            return self.board.render(None);
        }

        let blocks = self.current_piece.blocks(&self.shapes);

        self.board.render(Some((
            self.current_piece.row,
            self.current_piece.col,
            blocks,
        )))
    }
}
