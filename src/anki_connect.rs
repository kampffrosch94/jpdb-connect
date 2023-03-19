use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize)]
pub struct Response {
    pub result: Option<Box<dyn erased_serde::Serialize>>,
    pub error: Option<String>,
}

impl Response {
    #[allow(unused)]
    pub fn error(s: impl Into<String>) -> Self {
        Response {
            result: None,
            error: Some(s.into()),
        }
    }

    pub fn result(s: impl Serialize + 'static) -> Self {
        Response {
            result: Some(Box::new(s)),
            error: None,
        }
    }

    // we need this for compatibility with yomichan
    pub fn version_downgrade(&self) -> String {
        if let Some(r) = &self.result {
            return serde_json::to_string(r).unwrap();
        }
        if let Some(s) = &self.error {
            return format!(r#"{{"result": null, "error": "{}"}}"#, s);
        }
        "No response.".into()
    }
}

#[derive(Deserialize, Debug)]
pub struct AnkiConnectAction {
    pub action: String,
    pub version: i64,
    pub params: Option<Params>,
}

#[derive(Deserialize, Debug)]
pub struct Params {
    pub note: Option<Note>,
    pub query: Option<String>,
    pub notes: Option<Vec<Note>>,
}

#[derive(Deserialize, Debug)]
pub struct Note {
    pub fields: Fields,
}

#[derive(Deserialize, Debug)]
pub struct Fields {
    pub word: String,
    pub reading: String,
    pub sentence: String,
    pub definition: Option<String>,
}
