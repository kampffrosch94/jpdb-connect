use crate::parsing::{find_vocab_id, VocabId};
use crate::{anki_connect, parsing, Config};
use anyhow::{anyhow, Context as AnyhowContext, Result};
use log::*;
use reqwest::header::HeaderValue;
use reqwest::{Request, Response};
use std::collections::HashMap;
use std::fmt::Display;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tower::buffer::Buffer;
use tower::limit::{ConcurrencyLimit, RateLimit};
use tower::{Service, ServiceExt};

pub const DOMAIN: &str = "jpdb.io";
pub const URL_PREFIX: &str = "https://";

#[derive(Clone)]
pub struct JPDBConnection {
    pub service: BufferedService,
    pub config: Config,
}

type BufferedService = Buffer<ConcurrencyLimit<RateLimit<ReqwestService>>, Request>;

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

pub async fn send_request(service: &mut BufferedService, req: Request) -> Result<Response> {
    trace!("Request url: {}", req.url());
    service
        .ready()
        .await
        .map_err(|e| anyhow!("error getting reqwest client {e}"))?
        .call(req)
        .await
        .map_err(|e| anyhow!("{e}")) // we use this mapping to make our error type sized
}

fn abs_url(rel: impl Display) -> String {
    format!("{}{}{}", URL_PREFIX, DOMAIN, rel)
}

pub async fn get_request(service: &mut BufferedService, rel_url: &str) -> Result<Response> {
    let url = abs_url(rel_url);
    let req = Request::new(reqwest::Method::GET, reqwest::Url::parse(&url)?);
    send_request(service, req).await
}

pub async fn form_request(
    service: &mut BufferedService,
    rel_url: &str,
    payload: HashMap<&str, String>,
) -> Result<Response> {
    let url = abs_url(rel_url);
    let mut req = Request::new(reqwest::Method::POST, reqwest::Url::parse(&url)?);
    let payload = serde_urlencoded::ser::to_string(payload).context("encoding payload")?;
    *req.body_mut() = Some(reqwest::Body::from(payload));
    req.headers_mut().insert(
        "content-type",
        HeaderValue::from_static("application/x-www-form-urlencoded"),
    );
    send_request(service, req).await
}

impl JPDBConnection {
    pub async fn add_note(&mut self, s: &anki_connect::Fields) -> Result<()> {
        debug!("add W='{}' R='{}' S='{}'", s.word, s.reading, s.sentence);

        let url = format!("https://jpdb.io/search?q={}&lang=english#a", s.word);

        let req = Request::new(reqwest::Method::GET, reqwest::Url::parse(&url)?);
        let res = send_request(&mut self.service, req)
            .await
            .context("search request")?;
        let body = &res.text().await?;
        let detail_url = parsing::find_detail_url(body, &s.word, &s.reading);

        if self.config.auto_open {
            let url = if let Ok(ref rel_url) = &detail_url {
                format!("{}{}{}", URL_PREFIX, DOMAIN, rel_url)
            } else {
                info!("Can't find details page for: {}", s.word);
                url.into()
            };
            info!("Opening: {}", url);
            open::that(&url)?;
        }

        if self.config.session_id.is_some() {
            if let Ok(ref detail_url) = detail_url {
                // look up vocab id on details page
                let res = get_request(&mut self.service, detail_url)
                    .await
                    .context("get detail page")?;
                let body = &res.text().await?;
                trace!("Details page:");
                trace!("{}", body);
                let vocab = VocabCard { body };
                if let Some(deck_id) = self.config.auto_add {
                    info!("Adding card to deck: {}", abs_url(detail_url));
                    vocab
                        .add_to_deck(&mut self.service, deck_id, &detail_url)
                        .await?;
                }
                if self.config.auto_unlock {
                    info!("unlocking: {}", abs_url(detail_url));
                    vocab.force_unlock(&mut self.service, &detail_url).await?;
                }
                if self.config.auto_forq {
                    // it appears we don't need to check whether for FORQing is possible
                    info!("FORQing: {}", abs_url(detail_url));
                    vocab.forq(&mut self.service, &detail_url).await?;
                }
            } else {
                if self.config.auto_add.is_some() || self.config.auto_forq {
                    error!("Card can not be handled automatically, because it's detail page can not be found.");
                    return Err(anyhow::anyhow!("can't find card"));
                }
            }
        }
        Ok(())
    }
}

struct VocabCard<'a> {
    body: &'a str,
}

impl VocabCard<'_> {
    fn find_id(&self) -> Result<VocabId> {
        Ok(find_vocab_id(self.body).context("can't find vocab id")?)
    }

    async fn add_to_deck(
        &self,
        service: &mut BufferedService,
        deck_id: u64,
        origin: &str,
    ) -> Result<()> {
        let vocab_id = self.find_id()?;
        let add_url = format!("/deck/{}/add", deck_id);
        let mut payload = HashMap::new();
        payload.insert("v", vocab_id.v);
        payload.insert("r", vocab_id.r);
        payload.insert("s", vocab_id.s);
        payload.insert("origin", origin.to_string());

        let res = form_request(service, &add_url, payload)
            .await
            .context("add to deck")?;
        if !res.status().is_success() {
            return Err(anyhow!(
                "Add to deck failed, status: {}",
                res.status().as_u16()
            ));
        }
        Ok(())
    }

    async fn forq(&self, service: &mut BufferedService, origin: &str) -> Result<()> {
        let vocab_id = self.find_id()?;
        let mut payload = HashMap::new();
        payload.insert("v", vocab_id.v);
        payload.insert("s", vocab_id.s);
        payload.insert("origin", origin.to_string());
        let res = form_request(service, "/prioritize", payload)
            .await
            .context("forq request")?;
        let status = res.status();
        if !status.is_success() && !status.is_redirection() {
            debug!("Error body: {}", res.text().await.unwrap_or_default());
            return Err(anyhow!("FORQ failed, status: {}", status.as_u16()));
        }
        Ok(())
    }

    async fn force_unlock(&self, service: &mut BufferedService, origin: &str) -> Result<()> {
        let vocab_id = self.find_id()?;
        let mut payload = HashMap::new();
        payload.insert("v", vocab_id.v);
        payload.insert("s", vocab_id.s);
        payload.insert("origin", origin.to_string());
        let res = form_request(service, "/force-unlock", payload)
            .await
            .context("force-unlock request")?;
        let status = res.status();
        if !status.is_success() && !status.is_redirection() {
            debug!("Error body: {}", res.text().await.unwrap_or_default());
            return Err(anyhow!("force unlock failed, status: {}", status.as_u16()));
        }
        Ok(())
    }
}
