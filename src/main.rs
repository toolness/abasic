mod syntax_error;
mod tokenizer;

use rustyline::{error::ReadlineError, DefaultEditor};
use tokenizer::Tokenizer;

const HISTORY_FILENAME: &'static str = ".interpreter-history.txt";

fn run_interpreter() -> i32 {
    let Ok(mut rl) = DefaultEditor::new() else {
        eprintln!("Initializing DefaultEditor failed!");
        return 1;
    };

    // Ignore the result, if it errors it's generally b/c the file doesn't exist.
    let _ = rl.load_history(HISTORY_FILENAME);

    loop {
        let readline = rl.readline("] ");
        match readline {
            Ok(line) => {
                if let Err(err) = rl.add_history_entry(line.as_str()) {
                    eprintln!("WARNING: Failed to add history entry (${:?}).", err);
                }
                let tokenizer = Tokenizer::new(line);
                for token in tokenizer {
                    println!("Token: {:?}", token);
                }
            }
            Err(ReadlineError::Interrupted) => {
                eprintln!("CTRL-C pressed, exiting.");
                break;
            }
            Err(ReadlineError::Eof) => {
                break;
            }
            Err(err) => {
                eprintln!("Error: {:?}", err);
                return 1;
            }
        }
    }

    // Ignore the result, if we fail no biggie.
    let _ = rl.save_history(HISTORY_FILENAME);

    return 0;
}

fn main() {
    let exit_code = run_interpreter();
    std::process::exit(exit_code);
}
