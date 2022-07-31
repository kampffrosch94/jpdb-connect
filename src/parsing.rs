use chumsky::prelude::*;
use chumsky::text::digits;
use anyhow::{anyhow, Result};


pub fn find_detail_url(body: &str, vocab: &str, reading: &str) -> Result<String> {
    parse_detail_url(vocab, reading).parse(body).map_err(|e| anyhow!("{e:?}"))
}

fn parse_detail_url(vocab: &str, reading: &str) -> impl Parser<char, String, Error=Simple<char>> {
    take_until(
        just('"')
            .ignore_then(just("/vocabulary/"))
            .then(digits(10))
            .then(just(format!("/{vocab}/{reading}")))
            .map(|((a, b), c)| format!("{a}{b}{c}"))
    ).map(|(_a, b)| b)
}

pub fn substring(exclude: &str) -> impl Parser<char, String, Error=Simple<char>> + '_ {
    none_of(exclude).repeated().at_least(1).collect().boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_all_clean_test() {
        let example = r#"href or whatever idc "/vocabulary/1259620/見事/みごと?lang=english#a""#;
        // let parsed = parse_all().parse(example).unwrap();
        let parsed = parse_detail_url("見事", "みごと").parse(example).unwrap();
        assert_eq!("/vocabulary/1259620/見事/みごと", parsed)
    }
}