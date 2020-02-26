use regex::Regex;
use lsp_types::{Range, Position};

pub fn parse_range(msg: &str) -> Range {
    let re = Regex::new(r"\([^:]*:(\d+):(\d+)\)").unwrap();
    if let Some(cap) = re.captures(msg) {
        // zero-based in LSP
        let line = cap[1].parse::<u64>().unwrap() - 1;
        let character = cap[2].parse::<u64>().unwrap() - 1;
        Range {
            start: Position { line, character },
            end: Position { line, character: character + 1 },
        }
    } else {
        Range::default()
    }
}

#[test]
fn parse_range_test() {
    assert_eq!(parse_range("';' expected, but found identifier (<memory>:16:36)"),
    Range { start: Position { line: 15, character: 35 }, end: Position { line: 15, character: 36 }});
}