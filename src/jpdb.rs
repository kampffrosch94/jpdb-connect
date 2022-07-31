use std::collections::HashMap;
use crate::{anki_connect, Config, parsing};
use anyhow::Result;
use reqwest::Request;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use reqwest::header::HeaderValue;
use tower::buffer::Buffer;
use tower::limit::{ConcurrencyLimit, RateLimit};
use tower::{Service, ServiceExt};
use crate::parsing::find_vocab_id;

const DOMAIN: &str = "jpdb.io";
const URL_PREFIX: &str = "https://";

#[derive(Clone)]
pub struct JPDBConnection {
    pub service: Buffer<ConcurrencyLimit<RateLimit<ReqwestService>>, Request>,
    pub config: Config,
}

pub struct ReqwestService {
    pub client: reqwest::Client,
}

impl Service<Request> for ReqwestService {
    type Response = reqwest::Response;
    type Error = reqwest::Error;
    type Future =
    Pin<Box<dyn Future<Output=Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request) -> Self::Future {
        Box::pin(self.client.execute(req))
    }
}

impl JPDBConnection {
    pub async fn add_note(&mut self, s: &anki_connect::Fields) -> Result<()> {
        eprintln!("add W='{}' R='{}' S='{}'", s.word, s.reading, s.sentence);

        let url = format!("https://jpdb.io/search?q={}&lang=english#a", s.word);

        let req = Request::new(reqwest::Method::GET, reqwest::Url::parse(&url)?);
        let res = self.service.ready().await.unwrap().call(req).await.unwrap();
        let body = &res.text().await?;
        let detail_url = parsing::find_detail_url(body, &s.word, &s.reading);
        let had_detail = detail_url.is_ok();

        let url = if let Ok(ref path) = &detail_url {
            format!("{}{}{}", URL_PREFIX, DOMAIN, path)
        } else {
            url
        };

        if self.config.auto_open {
            open::that(&url)?;
        }

        if self.config.session_id.is_some() && had_detail {
            if let Some(deck_id) = self.config.auto_add {
                // look up vocab id on details page
                let req = Request::new(reqwest::Method::GET, reqwest::Url::parse(&url)?);
                let res = self.service.ready().await.unwrap().call(req).await.unwrap();
                let body = &res.text().await?;

                println!("{body}");
                let vocab_id = find_vocab_id(body)?;

                // add to deck
                let add_url = format!("{}{}/deck/{}/add", URL_PREFIX, DOMAIN, deck_id);
                let mut req = Request::new(reqwest::Method::POST, reqwest::Url::parse(&add_url)?);

                let payload = {
                    let mut payload = HashMap::new();
                    payload.insert("v", vocab_id.v);
                    payload.insert("r", vocab_id.r);
                    payload.insert("s", vocab_id.s);
                    payload.insert("origin", detail_url.unwrap());
                    serde_urlencoded::ser::to_string(payload).unwrap()
                };
                println!("{payload}");
                *req.body_mut() = Some(reqwest::Body::from(payload));
                req.headers_mut().insert("content-type",
                                         HeaderValue::from_static("application/x-www-form-urlencoded"),
                );


                let res = self.service.ready().await.unwrap().call(req).await.unwrap();

                assert!(res.status().is_success());
            }
        }

        Ok(())
    }
}
