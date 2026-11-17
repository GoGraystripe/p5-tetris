use crate::bot::*;

pub type MySinglePlayerBot = SimpleBot;

#[test]
fn create_bot_and_get_any_action_test() {
    let mut bot = MySinglePlayerBot::new();
    let _ = bot.get_move(&BoardState {
        tetromino_row: 20,
        tetromino_col: 4,
        tetromino_rotation: 0,
        tetromino: TetrominoType::J,
        board: [[CellState::Empty; 10]; 20],
    });
}
