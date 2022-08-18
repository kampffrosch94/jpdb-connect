use anyhow::{anyhow, Result};
use chumsky::prelude::*;
use chumsky::text::digits;

pub fn has_login_prompt(body: &str) -> bool {
    parse_login_prompt().parse(body).unwrap_or(false)
}

fn parse_login_prompt() -> impl Parser<char, bool, Error = Simple<char>> {
    take_until(just("https://jpdb.io/login_with_google")).map(|_| true)
}

pub fn find_detail_url(body: &str, vocab: &str, reading: &str) -> Result<String> {
    parse_detail_url(vocab, reading)
        .parse(body)
        .map_err(|e| anyhow!("{e:?}"))
}

fn parse_detail_url(vocab: &str, reading: &str) -> impl Parser<char, String, Error = Simple<char>> {
    take_until(
        just('"')
            .ignore_then(just("/vocabulary/"))
            .then(digits(10))
            .then(just(format!("/{vocab}/{reading}")))
            .map(|((a, b), c)| format!("{a}{b}{c}")),
    )
    .map(|(_a, b)| b)
}

#[derive(Debug)]
pub struct VocabId {
    pub v: String,
    pub s: String,
    pub r: String,
}

pub fn find_vocab_id(body: &str) -> Result<VocabId> {
    parse_vocab_id().parse(body).map_err(|e| anyhow!("{e:?}"))
}

fn parse_vocab_id() -> impl Parser<char, VocabId, Error = Simple<char>> {
    take_until(
        just(r#""/select_deck?v="#)
            .ignore_then(digits(10))
            .then_ignore(just("&amp;").or(just("&")))
            .then_ignore(just("s="))
            .then(digits(10))
            .then_ignore(just("&amp;").or(just("&")))
            .then_ignore(just("r="))
            .then(digits(10))
            .map(|((v, s), r)| VocabId { v, s, r }),
    )
    .map(|(_a, b)| b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_detail_url_test() {
        let example = r#"href or whatever idc "/vocabulary/1259620/見事/みごと?lang=english#a""#;
        let parsed = parse_detail_url("見事", "みごと").parse(example).unwrap();
        assert_eq!("/vocabulary/1259620/見事/みごと", parsed)
    }

    #[test]
    fn parse_vocab_id_test() {
        let example = r#" asdfafsdas "/select_deck?v=1414580&amp;s=1406264136&amp;r=1437918808""#;
        let parsed = parse_vocab_id().parse(example).unwrap();
        assert_eq!("1437918808", parsed.r);
    }
}
