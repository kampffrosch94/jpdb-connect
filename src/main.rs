use warp::Filter;
use warp::hyper::body::Bytes;
use serde::Deserialize;
use serde::Serialize;


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
            .map(|body: Bytes| {
                let s: String = String::from_utf8(body.slice(..).to_vec()).unwrap();
                eprintln!("{}", s);
                let a: AnkiConnectAction = serde_json::from_str(&s).unwrap();

                let answer = &handle_action(&a);
                return if a.version == 2 {
                    answer.version_downgrade()
                } else {
                    serde_json::to_string(answer).unwrap()
                }
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


fn handle_action(action: &AnkiConnectAction) -> Response {
    eprintln!("{:?}", &action);
    match action.action.as_str() {
        "version" => format!("6"),
        "deckNames" => format!(r#"["jpdb"]"#),
        "modelNames" => format!(r#"["jpdb"]"#),
        "modelFieldNames" => format!(r#"["word", "reading", "sentence"]"#),
        "addNote" => {
            let field = &action.params.as_ref().unwrap().note.as_ref().unwrap().fields;
            add_note(field);
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

fn add_note(s: &Fields) {
    eprintln!("add W='{}' R='{}' S='{}'", s.word, s.reading, s.sentence);

    // a[href*="人間/にんげん"]
}