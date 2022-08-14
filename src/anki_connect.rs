use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Debug)]
pub struct Response {
    pub result: Option<String>,
    pub error: Option<String>,
}

impl<T: Into<String>> From<T> for Response {
    fn from(s: T) -> Self {
        Response {
            result: Some(s.into()),
            error: None,
        }
    }
}

impl Response {
    #[allow(unused)]
    pub fn error(s: impl Into<String>) -> Self {
        Response {
            result: None,
            error: Some(s.into()),
        }
    }

    pub fn version_downgrade(&self) -> String {
        if let Some(s) = &self.result {
            return s.clone();
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
}
