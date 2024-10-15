mod anki_connect;
mod jpdb;
mod parsing;

use std::net::IpAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceBuilder;

use crate::anki_connect::{AnkiConnectAction, Response};
use crate::jpdb::*;
use crate::parsing::has_login_prompt;
use anyhow::{Context, Result};
use log::*;
use reqwest::cookie::Jar;
use tokio::sync::Mutex;
use warp::hyper::body::Bytes;
use warp::Filter;

#[derive(Clone, Debug, serde::Deserialize)]
pub struct Config {
    pub session_id: Option<String>,
    #[serde(default)]
    pub auto_open: bool,
    pub auto_add: Option<u64>,
    #[serde(default)]
    pub auto_forq: bool,
    #[serde(default)]
    pub auto_unlock: bool,
    #[serde(default)]
    pub auto_forget: bool,
    pub log_level: Option<Level>,
    #[serde(default)]
    pub add_mined_sentences: bool,
    #[serde(default)]
    pub add_custom_definition: bool,
    pub port: Option<u16>,
    pub ip: Option<String>,
}

impl Config {
    /// true if any options that need the user to be logged in and to access the detail page
    /// are enabled
    fn any_login_or_detail_options(&self) -> bool {
        self.auto_add.is_some()
            || self.auto_forq
            || self.auto_unlock
            || self.auto_forget
            || self.add_mined_sentences
            || self.add_custom_definition
    }
}

pub struct Cache {
    last_open: Option<String>,
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
    info!("Auto FORQ: {}", config.auto_forq);
    info!("Auto unlock: {}", config.auto_unlock);
    info!("Auto forget: {}", config.auto_forget);
    info!("Add mined sentences: {}", config.add_mined_sentences);
    info!("Add custom definition: {}", config.add_custom_definition);

    if !config.auto_open && !config.any_login_or_detail_options() {
        warn!("In this configuration jpdb-connect does not do anything.");
    }

    let test_login = config.session_id.is_some() && config.any_login_or_detail_options();
    if test_login {
        let response = client
            .get(if let Some(deck_id) = config.auto_add {
                abs_url(format!("/deck?id={}", deck_id))
            } else {
                abs_url("/")
            })
            .send()
            .await?;

        let status_code = response.status().as_u16();
        let body = &response
            .text()
            .await
            .unwrap_or("Some error happened.".into());
        let has_login_prompt = has_login_prompt(&body);
        debug!("has_login_prompt {}", has_login_prompt);
        trace!("Status code: {}", status_code);
        trace!("Body: {}", body);
        match (status_code, has_login_prompt) {
            (200, false) => info!("Login successful."),
            (200, true) => error!("Your sessionid is invalid. Update it to the one you currently use in your browser and try again."),
            (300..=399, _) => error!("Your sessionid is invalid. Update it to the one you currently use in your browser and try again."),
            (404, _) => error!("Your sessionid is invalid or the deck you are trying to add to does not exist."),
            _ => error!("Unhandled status code {status_code}."),
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
    let port = config.port.unwrap_or(3030);
    let ip = config
        .ip
        .as_ref()
        .and_then(|ip| IpAddr::from_str(&ip).ok())
        .unwrap_or([127, 0, 0, 1].into());

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

    let cache = Arc::new(Mutex::new(Cache { last_open: None }));

    let bytes = warp::any()
        .and(warp::body::bytes())
        .then(move |body: Bytes| {
            let jpdb = jpdb.clone();
            let mut cache = cache.clone();
            async move {
                let s: String = String::from_utf8(body.slice(..).to_vec()).unwrap();
                trace!("Request received:");
                trace!("{}", s);
                let a: AnkiConnectAction = serde_json::from_str(&s).unwrap();

                let answer = &handle_action(&a, jpdb, &mut cache).await;
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
    .run((ip, port))
    .await;
    Ok(())
}

async fn handle_action(
    action: &AnkiConnectAction,
    mut jpdb: JPDBConnection,
    cache: &mut Arc<Mutex<Cache>>,
) -> Response {
    debug!("{}", &action.action);
    match action.action.as_str() {
        "version" => Response::result(6),
        "deckNames" => Response::result(["jpdb"]),
        "modelNames" => Response::result(["jpdb", "Select to refresh"]),
        "modelFieldNames" => Response::result(["word", "reading", "sentence", "definition"]),
        "addNote" => {
            let field = &action
                .params
                .as_ref()
                .unwrap()
                .note
                .as_ref()
                .unwrap()
                .fields;
            let result = jpdb.add_note(field).await;
            {
                let mut cache = cache.lock().await;
                cache.last_open = match result {
                    Ok(ref s) => Some(s.clone()),
                    Err(_) => None,
                }
            }
            result
                .map(|_| Response::result(1234)) // TODO card id
                .map_err(|e| {
                    error!("{}", e.backtrace());
                    e
                })
                .unwrap_or_else(|e| Response::error(e.to_string()))
        }
        "guiBrowse" => {
            let cache = cache.lock().await;
            if let Some(ref open_url) = cache.last_open {
                match open::that(open_url)
                    .map(|_| Response::result(true))
                    .map_err(|e| {
                        error!("{}", e);
                        Response::error(e.to_string())
                    }) {
                    Ok(it) => it,
                    Err(it) => it,
                }
            } else {
                Response::error("Can't open nothing")
            }
        }
        "canAddNotes" => {
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
            warn!("Turn off duplicate detection or yomichan + jpdb-connect won't work.");
            //format!(r#"[{}]"#, v)
            Response::error("duplicate detection")
        }
        "storeMediaFile" => {
            warn!("unsupported action {}", action.action);
            Response::result("_hello.txt")
        }
        _ => {
            // multi
            // findnotes
            warn!("unsupported action {}", action.action);
            Response::error("unsupported action")
        }
    }
}
