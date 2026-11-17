mod board;
mod game;
mod tetromino;

use bot::{BoardState, CellState, SimpleBot, SinglePlayerBot, TetrisAction, TetrominoType};
use clap::Parser;
use game::{Config, Game};
use std::fs;
use std::path::PathBuf;
use tetromino::{load_pieces, load_wall_kicks};

mod bot;

#[cfg(test)]
mod tests;

#[derive(Parser)]
struct Args {
    #[arg(long, required_unless_present = "run_bot", conflicts_with = "run_bot")]
    simulate_inputs_single_player: Option<PathBuf>,

    #[arg(long)]
    run_bot: bool,

    #[arg(long)]
    resources: PathBuf,
}

fn main() {
    let args = Args::parse();

    // load the game config
    let config_path = args.resources.join("config.json");
    let config_file = fs::read_to_string(config_path).expect("Could not read config.json");
    let config: Config = serde_json::from_str(&config_file).expect("Invalid config.json");

    // load the tetromino shapes and wall kicks
    let shapes = load_pieces(&args.resources);
    let wall_kicks = load_wall_kicks(&args.resources);

    let mut game = Game::new(config, shapes, wall_kicks);

    if args.run_bot {
        run_bot(&mut game);
    } else if let Some(path) = args.simulate_inputs_single_player {
        // Process each input as one frame.
        let inputs = fs::read_to_string(path).expect("Could not read input file");
        for action in inputs.chars() {
            if !action.is_whitespace() {
                game.tick(action);
            }
        }
    }

    print!("{}", game.render());
}

fn run_bot(game: &mut Game) {
    let mut bot = SimpleBot::new();
    let mut frames = 0;

    while !game.game_over && game.lines_cleared < 40 && frames < 100_000 {
        let mut board = [[CellState::Empty; 10]; 20];
        for row in 0..20 {
            for col in 0..10 {
                if game.board.cells[row][col] {
                    board[row][col] = CellState::Filled;
                }
            }
        }

        let piece = match game.current_piece.piece_type.as_str() {
            "i" => TetrominoType::I,
            "j" => TetrominoType::J,
            "l" => TetrominoType::L,
            "o" => TetrominoType::O,
            "s" => TetrominoType::S,
            "z" => TetrominoType::Z,
            "t" => TetrominoType::T,
            _ => panic!("Unknown piece type"),
        };

        let state = BoardState {
            tetromino_row: game.current_piece.row as usize,
            tetromino_col: game.current_piece.col as usize,
            tetromino_rotation: game.current_piece.rotation,
            tetromino: piece,
            board,
        };

        let action = match bot.get_move(&state) {
            TetrisAction::L => 'L',
            TetrisAction::R => 'R',
            TetrisAction::C => 'C',
            TetrisAction::W => 'W',
            TetrisAction::D => 'D',
            TetrisAction::H => 'H',
            TetrisAction::E => 'E',
        };
        game.tick(action);
        frames += 1;
    }

    if game.game_over {
        println!("Bot topped out.");
    } else if game.lines_cleared >= 40 {
        println!("Bot reached 40 lines.");
    } else {
        println!("Stopped at the frame limit.");
    }
    println!("Lines cleared: {}", game.lines_cleared);
    println!("Frames: {}", frames);
}
