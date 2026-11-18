use clap::Parser;
use p5_tetris::bot::{MultiPlayerBot, SimpleBot, SinglePlayerBot, VersusBot};
use p5_tetris::game::Game;
use std::fs;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Parser)]
struct Args {
    #[arg(long)]
    simulate_inputs_single_player: Option<PathBuf>,

    #[arg(long)]
    simulate_inputs_multi_player: Vec<PathBuf>,

    #[arg(long)]
    run_bot: bool,

    #[arg(long)]
    single_threaded: bool,

    #[arg(long)]
    resources: Option<PathBuf>,

    #[arg(long)]
    list_bots: bool,

    #[arg(long)]
    trace_bot_run: bool,

    #[arg(long)]
    p1: Option<String>,

    #[arg(long)]
    p2: Option<String>,

    #[arg(long)]
    o1: Option<PathBuf>,

    #[arg(long)]
    o2: Option<PathBuf>,
}

fn main() {
    let args = Args::parse();
    let modes = [
        args.simulate_inputs_single_player.is_some(),
        !args.simulate_inputs_multi_player.is_empty(),
        args.run_bot,
        args.list_bots,
        args.trace_bot_run,
    ];
    let mut mode_count = 0;
    for selected in modes {
        if selected {
            mode_count += 1;
        }
    }
    if mode_count != 1 {
        argument_error("choose exactly one replay or bot mode");
    }
    if args.single_threaded && !args.run_bot {
        argument_error("--single-threaded requires --run-bot");
    }
    let has_trace_options =
        args.p1.is_some() || args.p2.is_some() || args.o1.is_some() || args.o2.is_some();
    if has_trace_options && !args.trace_bot_run {
        argument_error("--p1, --p2, --o1, and --o2 require --trace-bot-run");
    }

    if args.list_bots {
        println!("good-bot\nbad-bot");
        return;
    }

    let Some(resources) = args.resources.as_deref() else {
        argument_error("--resources is required");
    };
    if args.trace_bot_run {
        let (Some(p1), Some(p2), Some(o1), Some(o2)) = (
            args.p1.as_deref(),
            args.p2.as_deref(),
            args.o1.as_deref(),
            args.o2.as_deref(),
        ) else {
            argument_error("--trace-bot-run needs --p1, --p2, --o1, and --o2");
        };
        run_bot_match(resources, p1, p2, o1, o2);
        return;
    }

    if !args.simulate_inputs_multi_player.is_empty() {
        if args.simulate_inputs_multi_player.len() != 2 {
            argument_error("pass --simulate-inputs-multi-player once for each of the two players");
        }
        run_multiplayer(resources, &args.simulate_inputs_multi_player);
        return;
    }

    if args.run_bot {
        run_bot(resources, args.single_threaded);
        return;
    }

    let mut game = Game::load(resources);
    if let Some(path) = args.simulate_inputs_single_player {
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

fn argument_error(message: &str) -> ! {
    eprintln!("error: {message}");
    std::process::exit(2);
}

fn run_multiplayer(resources: &Path, paths: &[PathBuf]) {
    let mut player_one = Game::load(resources);
    let mut player_two = Game::load(resources);
    let input_one = fs::read_to_string(&paths[0]).expect("Could not read player one's inputs");
    let input_two = fs::read_to_string(&paths[1]).expect("Could not read player two's inputs");
    let mut actions_one = input_one.chars().filter(|action| !action.is_whitespace());
    let mut actions_two = input_two.chars().filter(|action| !action.is_whitespace());

    while !player_one.game_over && !player_two.game_over {
        let action_one = actions_one.next();
        let action_two = actions_two.next();
        if action_one.is_none() && action_two.is_none() {
            break;
        }

        // A player with no inputs left idles for this frame.
        Game::multiplayer_tick(
            &mut player_one,
            &mut player_two,
            action_one.unwrap_or('E'),
            action_two.unwrap_or('E'),
        );
    }

    print_match(&player_one, &player_two);
}

fn print_match(player_one: &Game, player_two: &Game) {
    let board_one = player_one.render();
    let board_two = player_two.render();
    for (row_one, row_two) in board_one.lines().zip(board_two.lines()) {
        println!("{row_one}    {row_two}");
    }
}

fn named_bot(name: &str, resources: &Path) -> VersusBot {
    match name {
        "good-bot" => VersusBot::new(resources),
        "bad-bot" => VersusBot::new_bad(resources),
        _ => argument_error("choose good-bot or bad-bot"),
    }
}

fn run_bot_match(resources: &Path, p1: &str, p2: &str, o1: &Path, o2: &Path) {
    if o1 == o2 {
        argument_error("use different trace paths");
    }

    let mut bot_one = named_bot(p1, resources);
    let mut bot_two = named_bot(p2, resources);
    let mut trace_one = String::new();
    let mut trace_two = String::new();

    loop {
        let (player_one, player_two) = bot_one.games();
        if player_one.game_over || player_two.game_over {
            write_traces(o1, o2, &trace_one, &trace_two).expect("Could not write match traces");
            print_match(player_one, player_two);
            break;
        }

        let action_one = bot_one.get_move();
        let action_two = bot_two.get_move();
        trace_one.push(action_one.as_char());
        trace_two.push(action_two.as_char());
        bot_one.update_with(&[action_one, action_two]);
        // Each bot tracks its own board first.
        bot_two.update_with(&[action_two, action_one]);
    }
}

fn write_traces(o1: &Path, o2: &Path, trace_one: &str, trace_two: &str) -> std::io::Result<()> {
    // Open both files before truncating either one.
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(false);
    let mut file_one = options.open(o1)?;
    let mut file_two = options.open(o2)?;

    #[cfg(unix)]
    let same_file = {
        let one = file_one.metadata()?;
        let two = file_two.metadata()?;
        one.dev() == two.dev() && one.ino() == two.ino()
    };
    #[cfg(not(unix))]
    let same_file = fs::canonicalize(o1)? == fs::canonicalize(o2)?;

    if same_file {
        argument_error("trace paths point to the same file");
    }

    file_one.set_len(0)?;
    file_one.write_all(trace_one.as_bytes())?;
    file_two.set_len(0)?;
    file_two.write_all(trace_two.as_bytes())
}

fn run_bot(resources: &Path, single_threaded: bool) {
    let mut bot = SimpleBot::with_threads(resources, !single_threaded);
    let mut frames = 0;
    let mut total_seconds = 0.0;
    let mut slowest_seconds = 0.0;
    let game_start = Instant::now();

    while !bot.game.game_over && bot.game.lines_cleared < 40 && frames < 100_000 {
        let start = Instant::now();
        let bot_action = bot.get_move();
        let seconds = start.elapsed().as_secs_f64();
        total_seconds += seconds;
        if seconds > slowest_seconds {
            slowest_seconds = seconds;
        }

        bot.update_with(bot_action);
        frames += 1;
    }

    let game_seconds = game_start.elapsed().as_secs_f64();

    let game = &bot.game;
    if game.game_over {
        println!("Bot topped out.");
    } else if game.lines_cleared >= 40 {
        println!("Bot reached 40 lines.");
    } else {
        println!("Stopped at the frame limit.");
    }
    println!("Lines cleared: {}", game.lines_cleared);
    println!("Frames: {}", frames);
    println!("Game time: {:.6} seconds", game_seconds);
    println!("Total bot time: {:.6} seconds", total_seconds);
    if frames > 0 {
        println!(
            "Average move: {:.3} ms",
            total_seconds / frames as f64 * 1000.0
        );
    }
    println!("Slowest move: {:.3} ms", slowest_seconds * 1000.0);
    print!("{}", game.render());
}
