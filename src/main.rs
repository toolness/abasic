mod syntax_error;
mod tokenizer;

use tokenizer::Tokenizer;

fn main() {
    let tokenizer = Tokenizer::new("PRINT \"HELLO WORLD\"");
    for token in tokenizer {
        println!("Token: {:?}", token);
    }
}
