mod board;
mod game;
mod tetromino;

use clap::Parser;
use game::{Config, Game};
use std::fs;
use std::path::PathBuf;
use tetromino::{load_pieces, load_wall_kicks};

#[derive(Parser)]
struct Args {
    #[arg(long)]
    simulate_inputs_single_player: PathBuf,

    #[arg(long)]
    resources: PathBuf,
}

fn main() {
    let args = Args::parse();

    // Load the game config
    let config_path = args.resources.join("config.json");
    let config_file = fs::read_to_string(config_path).expect("Could not read config.json");
    let config: Config = serde_json::from_str(&config_file).expect("Invalid config.json");

    // Load the tetromino shapes and wall kicks
    let shapes = load_pieces(&args.resources);
    let wall_kicks = load_wall_kicks(&args.resources);

    let mut game = Game::new(config, shapes, wall_kicks);

    // Process each of the inputs as a singular frame
    let inputs =
        fs::read_to_string(&args.simulate_inputs_single_player).expect("Could not read input file");

    for action in inputs.chars() {
        if !action.is_whitespace() {
            game.tick(action);
        }
    }

    print!("{}", game.render());
}
