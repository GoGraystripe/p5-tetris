use rand::SeedableRng;
use rand::seq::SliceRandom;
use rand_xoshiro::Xoshiro128PlusPlus;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Deserialize)]
pub struct PieceShape {
    pub states: [[[i32; 2]; 4]; 4],
    pub wall_kick: String,
}

pub type PieceShapes = HashMap<String, PieceShape>;
pub type WallKicks = HashMap<String, HashMap<String, Vec<[i32; 2]>>>;

pub fn load_pieces(resources: &Path) -> PieceShapes {
    let path = resources.join("tetrominoes.json");
    let contents = fs::read_to_string(path).expect("Could not read tetrominoes.json");

    serde_json::from_str(&contents).expect("Invalid tetrominoes.json")
}

pub fn load_wall_kicks(resources: &Path) -> WallKicks {
    let path = resources.join("wall_kicks.json");
    let contents = fs::read_to_string(path).expect("Could not read wall_kicks.json");

    serde_json::from_str(&contents).expect("Invalid wall_kicks.json")
}

pub struct Tetromino {
    pub piece_type: String,
    pub row: i32,
    pub col: i32,
    pub rotation: usize,
}

impl Tetromino {
    pub fn new(piece_type: String) -> Self {
        Self {
            piece_type,
            row: 20,
            col: 4,
            rotation: 0,
        }
    }

    // Get the four blocks in the current rotation
    pub fn blocks<'a>(&self, shapes: &'a PieceShapes) -> &'a [[i32; 2]; 4] {
        &shapes[&self.piece_type].states[self.rotation]
    }
}

pub struct PieceBag {
    pieces: Vec<String>,
    rng: Xoshiro128PlusPlus,
}

impl PieceBag {
    pub fn new(seed: [u8; 16]) -> Self {
        Self {
            pieces: Vec::new(),
            rng: Xoshiro128PlusPlus::from_seed(seed),
        }
    }

    pub fn next_piece(&mut self) -> String {
        if self.pieces.is_empty() {
            // Refill the bag when all seven pieces have been used
            self.pieces = ["i", "j", "l", "o", "s", "t", "z"]
                .iter()
                .map(|piece| piece.to_string())
                .collect();

            self.pieces.shuffle(&mut self.rng);
        }

        self.pieces.pop().unwrap()
    }
}
