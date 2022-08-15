mod anki_connect;
mod jpdb;
mod parsing;

use std::time::Duration;
use tower::ServiceBuilder;

use crate::anki_connect::{AnkiConnectAction, Response};
use crate::jpdb::*;
use anyhow::{Context, Result};
use log::*;
use reqwest::cookie::Jar;
use warp::hyper::body::Bytes;
use warp::Filter;

#[derive(Clone, Debug, serde::Deserialize)]
pub struct Config {
    pub session_id: Option<String>,
    pub auto_open: bool,
    pub auto_add: Option<u64>,
    pub log_level: Option<Level>,
}

fn read_config() -> Result<Config> {
    let exe_path = std::env::current_exe()?;
    let config_path = exe_path
        .parent()
        .context("no parent T_T")?
        .join("jpdb_connect.toml");

    let content = if config_path.as_path().exists() {
        println!("loading config from {}", config_path.display());
        std::fs::read_to_string(&config_path)?
    } else {
        println!("creating default config file at {}", config_path.display());
        let s = include_str!("default_config.toml");
        std::fs::write(&config_path, s)?;
        s.to_string()
    };
    Ok(toml::from_str(&content)?)
}

async fn validate_config(config: &Config, client: &reqwest::Client) -> Result<()> {
    let should_auto_add = config.session_id.is_some() && config.auto_add.is_some();

    info!("Auto open card in browser: {}", config.auto_open);
    info!("Auto add card to deck: {}", should_auto_add);

    if !config.auto_open && !should_auto_add {
        warn!("In this configuration jpdb-connect does not do anything.");
    }

    if should_auto_add {
        let response = client
            .get(format!(
                "{}{}/deck?id={}",
                URL_PREFIX,
                DOMAIN,
                config.auto_add.unwrap()
            ))
            .send()
            .await?;

        let status_code = response.status().as_u16();
        match status_code {
            200 => info!("Login successful."),
            300..=399 => error!("Your sessionid is invalid. Update it to the one you currently use in your browser and try again."),
            404 => error!("Your sessionid is invalid or the deck you are trying to add to does not exist."),
            _ => error!("Unhandled status code {status_code}"),
        }
    }

    Ok(())
}

fn setup_logger(config: &Config) -> Result<()> {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{}[{}][{}] {}",
                chrono::Local::now().format("[%H:%M:%S]"),
                record.target(),
                record.level(),
                message
            ))
        })
        .level(LevelFilter::Info)
        .level_for(
            "jpdb_connect",
            config.log_level.unwrap_or(Level::Info).to_level_filter(),
        )
        .chain(std::io::stdout())
        //.chain(fern::log_file("output.log")?)
        .apply()?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = read_config().context("Config file can not be loaded.")?;
    setup_logger(&config)?;

    let mut client = reqwest::Client::builder();
    if let Some(ref sid) = config.session_id {
        let jar = Jar::default();
        const COOKIE_NAME: &str = "sid";
        let cookie_str = format!("{COOKIE_NAME}={}; Domain={DOMAIN}", sid);
        jar.add_cookie_str(&cookie_str, &format!("{URL_PREFIX}{DOMAIN}").parse()?);
        client = client.cookie_store(true).cookie_provider(jar.into());
    }
    let client = client.build()?;

    validate_config(&config, &client).await?;

    let service = ServiceBuilder::new()
        .buffer(100)
        .concurrency_limit(1)
        .rate_limit(5, Duration::from_secs(3)) // so that we don't get IP banned
        .service(ReqwestService { client });

    let jpdb = JPDBConnection { service, config };

    let bytes = warp::any()
        .and(warp::body::bytes())
        .then(move |body: Bytes| {
            let jpdb = jpdb.clone();
            async move {
                let s: String = String::from_utf8(body.slice(..).to_vec()).unwrap();
                trace!("Request received:");
                trace!("{}", s);
                let a: AnkiConnectAction = serde_json::from_str(&s).unwrap();

                let answer = &handle_action(&a, jpdb).await;
                let r = if a.version == 2 {
                    answer.version_downgrade()
                } else {
                    serde_json::to_string(answer).unwrap()
                };
                debug!("Anki-connect answer: '{}'", r);
                return r;
            }
        });

    info!("Starting server.");
    warp::serve(bytes.with(warp::log::custom(|info| {
        debug!("{} {} {}", info.method(), info.path(), info.status(),);
    })))
    .run(([127, 0, 0, 1], 3030))
    .await;
    Ok(())
}

async fn handle_action(
    action: &AnkiConnectAction,
    mut jpdb: JPDBConnection,
) -> Response {
    debug!("{}", &action.action);
    match action.action.as_str() {
        "version" => Response::result(6),
        "deckNames" => Response::result(["jpdb"]),
        "modelNames" => Response::result(["jpdb"]),
        "modelFieldNames" => Response::result(["word", "reading", "sentence"]),
        "addNote" => {
            let field = &action
                .params
                .as_ref()
                .unwrap()
                .note
                .as_ref()
                .unwrap()
                .fields;
            jpdb.add_note(field)
                .await
                .map(|_| Response::result(1234)) // TODO card id
                .unwrap_or_else(|e| Response::error(e.to_string()))
        }
        "guiBrowse" => {
            // TODO open browser
            // action.params.query = "nid:1234"
            Response::error("unsupported")
        }
        "canAddNotes" => {
            // TODO return vec of bools
            let _v = action
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
            warn!("WIP");
            //format!(r#"[{}]"#, v)
            Response::error("unsupported")
        }
        _ => {
            // multi
            // findnotes
            warn!("unsupported action {}", action.action);
            Response::error("unsupported")
        }
    }
}
