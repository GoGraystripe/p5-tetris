use p5_tetris::bot::{MultiPlayerBot, SimpleBot, SinglePlayerBot, TetrisAction, VersusBot};
use std::path::Path;

pub type MySinglePlayerBot = SimpleBot;
pub type MyMultiPlayerBot = VersusBot;

#[test]
fn create_bot_and_get_any_action_test() {
    let mut bot = MySinglePlayerBot::new(Path::new("resources"));
    let action = bot.get_move();
    bot.update_with(action);
}

#[test]
fn create_mut_bot_and_get_any_action_test() {
    let mut bot = MyMultiPlayerBot::new(Path::new("resources"));
    let action = bot.get_move();
    bot.update_with(&[action, TetrisAction::E]);
}
