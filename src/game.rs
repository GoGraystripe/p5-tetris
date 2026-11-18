use crate::board::{Board, HEIGHT, TOTAL_HEIGHT, WIDTH};
use crate::bot::GarbageLine;
use crate::tetromino::{PieceBag, PieceShapes, Tetromino, WallKicks};
use rand::{Rng, SeedableRng};
use rand_xoshiro::Xoshiro128PlusPlus;
use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Deserialize)]
pub struct Config {
    pub gravity: u32,
    pub gravity_decrease: u32,
    pub lock_delay: u32,
    pub random_seed: [u8; 16],
    #[serde(default)]
    pub garbage_queue_time: usize,
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
    pub lines_cleared: usize,
    pub garbage: Vec<GarbageLine>,
    garbage_queue_time: usize,
    garbage_rng: Xoshiro128PlusPlus,
}

impl Game {
    pub fn load(resources: &Path) -> Self {
        let config_path = resources.join("config.json");
        let config_file = fs::read_to_string(config_path).expect("Could not read config.json");
        let config: Config = serde_json::from_str(&config_file).expect("Invalid config.json");
        let shapes = crate::tetromino::load_pieces(resources);
        let wall_kicks = crate::tetromino::load_wall_kicks(resources);
        Self::new(config, shapes, wall_kicks)
    }

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
            lines_cleared: 0,
            garbage: Vec::new(),
            garbage_queue_time: config.garbage_queue_time,
            garbage_rng: Xoshiro128PlusPlus::from_seed(config.random_seed),
        }
    }

    pub fn multiplayer_tick(one: &mut Self, two: &mut Self, action_one: char, action_two: char) {
        let lines_one = one.lines_cleared;
        let lines_two = two.lines_cleared;
        let locked_one = one.tick(action_one);
        let locked_two = two.tick(action_two);

        // Cancel old garbage before either player receives new garbage.
        let send_one = one.cancel_garbage(one.lines_cleared - lines_one);
        let send_two = two.cancel_garbage(two.lines_cleared - lines_two);
        one.queue_garbage(send_two);
        two.queue_garbage(send_one);
        if locked_one {
            one.apply_garbage();
        }
        if locked_two {
            two.apply_garbage();
        }
        one.count_down_garbage();
        two.count_down_garbage();
    }

    fn cancel_garbage(&mut self, lines_cleared: usize) -> usize {
        if lines_cleared < 2 {
            return 0;
        }
        let mut rows = lines_cleared - 1;
        while rows > 0 && !self.garbage.is_empty() {
            let canceled = rows.min(self.garbage[0].num_rows_to_create);
            self.garbage[0].num_rows_to_create -= canceled;
            rows -= canceled;
            if self.garbage[0].num_rows_to_create == 0 {
                self.garbage.remove(0);
            }
        }
        rows
    }

    fn queue_garbage(&mut self, rows: usize) {
        if rows > 0 && !self.game_over {
            self.garbage.push(GarbageLine {
                num_rows_to_create: rows,
                ticks_till_applied: self.garbage_queue_time,
            });
        }
    }

    fn apply_garbage(&mut self) {
        while !self.game_over && !self.garbage.is_empty() && self.garbage[0].ticks_till_applied == 0 {
            let batch = self.garbage.remove(0);
            let hole = self.garbage_rng.next_u32() as usize % WIDTH;

            for _ in 0..batch.num_rows_to_create {
                for row in (1..TOTAL_HEIGHT).rev() {
                    self.board.cells[row] = self.board.cells[row - 1];
                }
                self.board.cells[0] = [true; WIDTH];
                self.board.cells[0][hole] = false;

                if self.board.cells[HEIGHT].contains(&true) {
                    self.game_over = true;
                    break;
                }
            }
        }
    }

    fn count_down_garbage(&mut self) {
        for batch in &mut self.garbage {
            if batch.ticks_till_applied > 0 {
                batch.ticks_till_applied -= 1;
            }
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

        self.lines_cleared += self.board.remove_full_rows();

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

    // Return whether the current piece locked this frame.
    pub fn tick(&mut self, action: char) -> bool {
        if self.game_over {
            return false;
        }

        match action {
            'L' => self.move_left(),
            'R' => self.move_right(),
            'C' => self.rotate(true),
            'W' => self.rotate(false),
            'H' => {
                self.hard_drop();
                return true;
            }
            'D' | 'E' => {}
            _ => {}
        }

        self.gravity_timer += 1;

        // Soft drop temporarily reduces the gravity interval, but does not reset the gravity timer
        let gravity_interval = if action == 'D' {
            if self.gravity > self.gravity_decrease {
                self.gravity - self.gravity_decrease
            } else {
                1
            }
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
                return true;
            }
        } else {
            self.lock_timer = 0;
        }
        false
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
