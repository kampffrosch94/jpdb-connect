mod anki_connect;
mod jpdb;

use std::time::Duration;
use tower::ServiceBuilder;

use crate::anki_connect::AnkiConnectAction;
use crate::jpdb::*;
use anyhow::Result;
use warp::hyper::body::Bytes;
use warp::Filter;

#[tokio::main]
async fn main() -> Result<()> {
    let client = reqwest::Client::builder()
        //.cookie_store(true)
        //.cookie_provider(jar.into())
        .build()?;
    let service = ServiceBuilder::new()
        .buffer(100)
        .concurrency_limit(1)
        .rate_limit(5, Duration::from_secs(3)) // so that we don't get IP banned
        .service(ReqwestService { client });

    let jpdb = JPDBConnection { service };

    let bytes = warp::any()
        .and(warp::body::bytes())
        .then(move |body: Bytes| {
            let jpdb = jpdb.clone();
            async move {
                let s: String = String::from_utf8(body.slice(..).to_vec()).unwrap();
                eprintln!("{}", s);
                let a: AnkiConnectAction = serde_json::from_str(&s).unwrap();

                let answer = &handle_action(&a, jpdb).await;
                return if a.version == 2 {
                    answer.version_downgrade()
                } else {
                    serde_json::to_string(answer).unwrap()
                };
            }
        });

    warp::serve(bytes.with(warp::log::custom(|info| {
        eprintln!("{} {} {}", info.method(), info.path(), info.status(),);
    })))
    .run(([127, 0, 0, 1], 3030))
    .await;
    Ok(())
}

async fn handle_action(
    action: &AnkiConnectAction,
    mut jpdb: JPDBConnection,
) -> anki_connect::Response {
    eprintln!("{}", &action.action);
    match action.action.as_str() {
        "version" => format!("6"),
        "deckNames" => format!(r#"["jpdb"]"#),
        "modelNames" => format!(r#"["jpdb"]"#),
        "modelFieldNames" => format!(r#"["word", "reading", "sentence"]"#),
        "addNote" => {
            let field = &action
                .params
                .as_ref()
                .unwrap()
                .note
                .as_ref()
                .unwrap()
                .fields;
            jpdb.add_note(field).await.unwrap();
            format!(r#"12345"#) // TODO some id
        }
        "guiBrowse" => {
            // TODO open browser
            // action.params.query = "nid:1234"
            format!(r#"ok"#)
        }
        "canAddNotes" => {
            // TODO return vec of bools
            let v = action
                .params
                .as_ref()
                .unwrap()
                .notes
                .as_ref()
                .unwrap()
                .iter()
                .map(|_| "true".to_string())
                .collect::<Vec<String>>()
                .join(", ");
            format!(r#"[{}]"#, v)
        }
        _ => {
            // multi
            // findnotes
            eprintln!("{}", action.action);
            format!("unsupported")
        }
    }
    .into()
}
