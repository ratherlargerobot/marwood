use crate::syntax::ReplHighlighter;
use log::trace;
use marwood::cell::Cell;
use marwood::lex::{scan, Token};
use marwood::parse::parse;
use marwood::vm::Vm;
use marwood::{lex, parse};
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::validate::{ValidationContext, ValidationResult, Validator};
use rustyline::{Editor, Result};
use rustyline_derive::{Completer, Helper, Hinter};
use std::borrow::Cow::Owned;

mod syntax;

#[derive(Completer, Helper, Hinter)]
struct InputValidator {
    highlighter: ReplHighlighter,
}

impl Validator for InputValidator {
    fn validate(&self, ctx: &mut ValidationContext) -> Result<ValidationResult> {
        match scan(ctx.input()) {
            Ok(tokens) => match parse(ctx.input(), &mut tokens.iter().peekable()) {
                Ok(_) => Ok(ValidationResult::Valid(None)),
                Err(parse::Error::Eof) => Ok(ValidationResult::Incomplete),
                Err(_) => Ok(ValidationResult::Valid(None)),
            },
            Err(lex::Error::Eof) => Ok(ValidationResult::Incomplete),
            Err(_) => Ok(ValidationResult::Valid(None)),
        }
    }
}

impl Highlighter for InputValidator {
    fn highlight<'l>(&self, line: &'l str, pos: usize) -> std::borrow::Cow<'l, str> {
        Owned(
            self.highlighter
                .highlight(line, pos)
                .replace("[1;34m", "[4m"),
        )
    }

    fn highlight_char(&self, line: &str, pos: usize) -> bool {
        self.highlighter.highlight_check(line, pos + 1)
    }
}

fn main() {
    pretty_env_logger::init();
    let validator = InputValidator {
        highlighter: ReplHighlighter::new(),
    };
    let mut rl = Editor::new();
    rl.set_helper(Some(validator));
    let mut remaining = "".to_string();

    let mut vm = Vm::new();
    loop {
        let readline = rl.readline_with_initial("> ", (&remaining, ""));
        match readline {
            Ok(line) => {
                rl.add_history_entry(line.as_str());
                remaining = parse_and_eval(&mut vm, &line).trim().to_string();
            }
            Err(ReadlineError::Interrupted | ReadlineError::Eof) => break,
            Err(err) => {
                eprintln!("error: {:#?}", err);
                break;
            }
        }
    }
}

/// Evaluate one expression from the input text and return
/// any text that was not evaluated.
fn parse_and_eval<'a>(vm: &mut Vm, text: &'a str) -> &'a str {
    // Tokenize the entire input
    let tokens = match scan(text) {
        Ok(tokens) => tokens,
        Err(e) => {
            println!("error: {}", e);
            return "";
        }
    };

    // Parse one expression from the token stream
    let mut cur = tokens.iter().peekable();
    trace!("lexer: {:?}", tokens);
    match parse(text, &mut cur) {
        Ok(cell) => {
            trace!("parser: {}", cell);
            match vm.eval(&cell) {
                Ok(Cell::Void) => {
                    println!();
                }
                Ok(cell) => {
                    println!("{:#}", cell);
                }
                Err(e) => {
                    println!("error: {}", e);
                }
            };
        }
        Err(e) => {
            println!("error: {}", e);
        }
    }

    // Given the position of the next token in the stream, determine
    // how far the parser consumed and return the remaining text
    match cur.peek() {
        Some(Token { span, .. }) => &text[span.0..],
        None => "",
    }
}
