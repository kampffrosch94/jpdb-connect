use warp::Filter;
use warp::hyper::body::Bytes;
use serde::Deserialize;
use serde::Serialize;
use anyhow::{anyhow, Context, Result};
use reqwest::Client;

const DOMAIN: &str = "jpdb.io";
const URL_PREFIX: &str = "https://";

#[derive(Serialize, Debug)]
struct Response {
    response: Option<String>,
    error: Option<String>,
}

impl<T: Into<String>> From<T> for Response {
    fn from(s: T) -> Self {
        Response {
            response: Some(s.into()),
            error: None,
        }
    }
}

impl Response {
    fn error(s: impl Into<String>) -> Self {
        Response {
            response: None,
            error: Some(s.into()),
        }
    }

    fn version_downgrade(&self) -> String {
        if let Some(s) = &self.response {
            return s.clone();
        }
        if let Some(s) = &self.error {
            return s.clone();
        }
        return "No response.".into();
    }
}


#[derive(Deserialize, Debug)]
struct AnkiConnectAction {
    action: String,
    version: i64,
    params: Option<Params>,
}


#[derive(Deserialize, Debug)]
struct Params {
    note: Option<Note>,
    query: Option<String>,
    notes: Option<Vec<Note>>,
}

#[derive(Deserialize, Debug)]
struct Note {
    fields: Fields,
}

#[derive(Deserialize, Debug)]
struct Fields {
    word: String,
    reading: String,
    sentence: String,
}

#[tokio::main]
async fn main() {
    let bytes =
        warp::any().
            and(warp::body::bytes())
            .then(|body: Bytes| async move{
                let s: String = String::from_utf8(body.slice(..).to_vec()).unwrap();
                eprintln!("{}", s);
                let a: AnkiConnectAction = serde_json::from_str(&s).unwrap();

                let answer = &handle_action(&a).await;
                return if a.version == 2 {
                    answer.version_downgrade()
                } else {
                    serde_json::to_string(answer).unwrap()
                };
            });


    warp::serve(bytes.
        with(warp::log::custom(|info| {
            eprintln!(
                "{} {} {}",
                info.method(),
                info.path(),
                info.status(),
            );
        }))
    )
        .run(([127, 0, 0, 1], 3030))
        .await;
}


async fn handle_action(action: &AnkiConnectAction) -> Response {
    eprintln!("{:?}", &action);
    match action.action.as_str() {
        "version" => format!("6"),
        "deckNames" => format!(r#"["jpdb"]"#),
        "modelNames" => format!(r#"["jpdb"]"#),
        "modelFieldNames" => format!(r#"["word", "reading", "sentence"]"#),
        "addNote" => {
            let field = &action.params.as_ref().unwrap().note.as_ref().unwrap().fields;
            add_note(field).await.unwrap();
            format!(r#"12345"#) // TODO some id
        }
        "guiBrowse" => {
            // TODO open browser
            // action.params.query = "nid:1234"
            format!(r#"ok"#)
        }
        "canAddNotes" => {
            // TODO return vec of bools
            format!(r#"[true]"#)
        }
        _ => {
            // multi
            // findnotes
            eprintln!("{}", action.action);
            format!("unsupported")
        }
    }.into()
}

async fn add_note(s: &Fields) -> Result<()> {
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
        .next();

    let url = if let Some(selected) = selected {
        let path = selected.value().attr("href").context("should be impossible")?;
        format!("{}{}{}", URL_PREFIX, DOMAIN, path)
    } else {
        url
    };

    let url = url.split("?").next().unwrap(); // cut off parameters and jump points
    open::that(url)?;
    Ok(())
}