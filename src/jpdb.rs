use crate::anki_connect;
use anyhow::{anyhow, Result};
use reqwest::Request;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tower::buffer::Buffer;
use tower::limit::{ConcurrencyLimit, RateLimit};
use tower::{Service, ServiceExt};

const DOMAIN: &str = "jpdb.io";
const URL_PREFIX: &str = "https://";

#[derive(Clone)]
pub struct JPDBConnection {
    pub service: Buffer<ConcurrencyLimit<RateLimit<ReqwestService>>, Request>,
}

pub struct ReqwestService {
    pub client: reqwest::Client,
}

impl Service<Request> for ReqwestService {
    type Response = reqwest::Response;
    type Error = reqwest::Error;
    type Future =
        Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

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

        let req = Request::new(reqwest::Method::POST, reqwest::Url::parse(&url)?);
        let res = self.service.ready().await.unwrap().call(req).await.unwrap();
        let body = &res.text().await?;
        let document = scraper::Html::parse_document(&body);
        let selector = scraper::Selector::parse(&format!(r#"a[href*="{}/{}"]"#, s.word, s.reading))
            .map_err(|e| anyhow!("{e:?}"))?;
        let selected = document
            .select(&selector)
            .map(|v| v.value().attr("href").unwrap())
            .find(|s| s.contains("vocabulary"));

        let url = if let Some(path) = selected {
            format!("{}{}{}", URL_PREFIX, DOMAIN, path)
        } else {
            url
        };

        let url = url.split('#').next().unwrap(); // cut off jump points
        open::that(url)?;
        Ok(())
    }
}
