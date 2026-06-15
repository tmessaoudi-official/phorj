use std::process::exit;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("lex") => {
            let path = args.get(2).unwrap_or_else(|| { eprintln!("usage: phorge lex <file>"); exit(2); });
            let src = std::fs::read_to_string(path).unwrap_or_else(|e| { eprintln!("read error: {e}"); exit(1); });
            match phorge::lexer::lex(&src) {
                Ok(toks) => for t in toks { println!("{:?} @ {}:{}", t.kind, t.span.line, t.span.col); }
                Err(e) => { eprintln!("lex error at {}:{}: {}", e.line, e.col, e.message); exit(1); }
            }
        }
        _ => { eprintln!("usage: phorge lex <file>"); exit(2); }
    }
}
