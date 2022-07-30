use crate::anki_connect;
use anyhow::{anyhow, Result};
use reqwest::Client;

const DOMAIN: &str = "jpdb.io";
const URL_PREFIX: &str = "https://";

pub async fn add_note(s: &anki_connect::Fields) -> Result<()> {
    eprintln!("add W='{}' R='{}' S='{}'", s.word, s.reading, s.sentence);

    let url = format!("https://jpdb.io/search?q={}&lang=english#a", s.word);

    let client = Client::builder()
        //.cookie_store(true)
        //.cookie_provider(jar.into())
        .build()?;

    let req = client.get(&url).build()?;
    let body = client.execute(req).await?.text().await?;
    let document = scraper::Html::parse_document(&body);
    let selector = scraper::Selector::parse(&format!(r#"a[href*="{}/{}"]"#, s.word, s.reading))
        .map_err(|e| anyhow!("{e:?}"))?;
    let selected = document
        .select(&selector)
        .map(|v| v.value().attr("href").unwrap())
        .filter(|s| s.contains("vocabulary"))
        .next();

    let url = if let Some(path) = selected {
        format!("{}{}{}", URL_PREFIX, DOMAIN, path)
    } else {
        url
    };

    let url = url.split("#").next().unwrap(); // cut off jump points
    open::that(url)?;
    Ok(())
}
