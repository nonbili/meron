use std::collections::HashSet;
use std::io::Read;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail, ensure};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use ureq::RequestExt as _;
use url::Url;

use crate::store::Task;

#[derive(Clone, Deserialize)]
pub(super) struct List {
    pub id: String,
    pub title: String,
}

#[derive(Clone, Deserialize)]
pub(super) struct Item {
    pub id: String,
    pub etag: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub status: String,
    pub due: Option<String>,
    pub completed: Option<String>,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub deleted: bool,
}

impl Item {
    pub fn due_at(&self) -> i64 {
        self.due
            .as_deref()
            .and_then(parse_timestamp)
            .and_then(|at| date_at_local_midnight(at.date_naive()))
            .unwrap_or_default()
    }

    pub fn completed_at(&self) -> i64 {
        if self.status == "completed" {
            self.completed
                .as_deref()
                .and_then(parse_timestamp)
                .map_or(1, |at| at.timestamp().max(1))
        } else {
            0
        }
    }
}

fn parse_timestamp(value: &str) -> Option<chrono::DateTime<chrono::FixedOffset>> {
    chrono::DateTime::parse_from_rfc3339(value).ok()
}

pub(super) fn date_at_local_midnight(date: chrono::NaiveDate) -> Option<i64> {
    use chrono::TimeZone as _;
    // Some timezones skip midnight on a DST change.
    (0..24).find_map(|hour| {
        chrono::Local
            .from_local_datetime(&date.and_hms_opt(hour, 0, 0)?)
            .earliest()
            .map(|at| at.timestamp())
    })
}

fn date_string(at: i64) -> Option<String> {
    (at > 0)
        .then(|| chrono::DateTime::from_timestamp(at, 0))
        .flatten()
        .map(|date| {
            date.with_timezone(&chrono::Local)
                .format("%Y-%m-%dT00:00:00Z")
                .to_string()
        })
}

pub(super) fn payload(task: &Task) -> Value {
    json!({
        "title": task.title,
        "notes": task.notes,
        "due": date_string(task.due_at),
        "status": if task.completed_at > 0 { "completed" } else { "needsAction" },
    })
}

pub(super) struct Client {
    agent: ureq::Agent,
    endpoint: Url,
    authorization: String,
}

#[derive(Debug)]
pub(super) struct HttpStatus(pub u16, Option<&'static str>);

impl std::fmt::Display for HttpStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.1 {
            Some(message) => f.write_str(message),
            None => write!(f, "Google Tasks returned HTTP {}", self.0),
        }
    }
}
impl std::error::Error for HttpStatus {}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", bound(deserialize = "T: Deserialize<'de>"))]
struct Page<T> {
    kind: String,
    next_page_token: Option<String>,
    #[serde(default)]
    items: Vec<T>,
}

impl Client {
    pub fn new(token: &str) -> Result<Self> {
        Ok(Self {
            agent: crate::proxy::agent()?,
            endpoint: Url::parse("https://tasks.googleapis.com/tasks/v1/")?,
            authorization: format!("Bearer {token}"),
        })
    }

    #[cfg(test)]
    pub fn with_endpoint(mut self, endpoint: Url) -> Self {
        self.endpoint = endpoint;
        self
    }

    fn url(&self, parts: &[&str]) -> Result<Url> {
        let mut url = self.endpoint.clone();
        for part in parts {
            ensure!(
                !part.is_empty() && !matches!(*part, "." | ".."),
                "Invalid Google Tasks resource ID"
            );
        }
        url.path_segments_mut()
            .map_err(|_| anyhow!("Invalid Google Tasks endpoint"))?
            .pop_if_empty()
            .extend(parts.iter().copied());
        Ok(url)
    }

    fn request(
        &self,
        method: &str,
        url: &Url,
        etag: &str,
        body: Option<&Value>,
    ) -> Result<Vec<u8>> {
        let mut req = ureq::http::Request::builder()
            .method(method)
            .uri(url.as_str())
            .header("Authorization", &self.authorization);
        if !etag.is_empty() {
            let version = if etag.starts_with('"') || etag.starts_with("W/\"") {
                etag.to_string()
            } else {
                format!("\"{etag}\"")
            };
            req = req.header("If-Match", version);
        }
        if body.is_some() {
            req = req.header("Content-Type", "application/json");
        }
        let bytes = body
            .map(serde_json::to_vec)
            .transpose()?
            .unwrap_or_default();
        let mut res = req
            .body(bytes)?
            .with_agent(&self.agent)
            .configure()
            .max_redirects(0)
            .http_status_as_error(false)
            .timeout_connect(Some(Duration::from_secs(10)))
            .timeout_global(Some(Duration::from_secs(30)))
            .run()
            .context("Could not reach Google Tasks")?;
        if !res.status().is_success() {
            let status = res.status().as_u16();
            if status == 401 {
                return Err(crate::google_grant::Rejected(
                    "Reconnect Google Tasks to resume syncing",
                )
                .into());
            }
            if status == 403 {
                let mut body = String::new();
                res.body_mut()
                    .as_reader()
                    .take(16 * 1024)
                    .read_to_string(&mut body)?;
                if body.contains("SERVICE_DISABLED") || body.contains("accessNotConfigured") {
                    return Err(HttpStatus(status, Some("Enable the Google Tasks API in the Google Cloud project used by Meron.")).into());
                }
            }
            return Err(HttpStatus(status, None).into());
        }
        let mut bytes = Vec::new();
        res.body_mut()
            .as_reader()
            .take(2 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() <= 2 * 1024 * 1024,
            "Google Tasks response exceeded the size limit"
        );
        Ok(bytes)
    }

    pub fn lists(&self) -> Result<Vec<List>> {
        self.pages(self.url(&["users", "@me", "lists"])?, "tasks#taskLists")
    }

    pub fn items(&self, list: &str) -> Result<Vec<Item>> {
        let mut url = self.url(&["lists", list, "tasks"])?;
        url.query_pairs_mut()
            .append_pair("showCompleted", "true")
            .append_pair("showHidden", "true");
        self.pages(url, "tasks#tasks")
    }

    fn pages<T: DeserializeOwned>(&self, mut url: Url, kind: &str) -> Result<Vec<T>> {
        url.query_pairs_mut().append_pair("maxResults", "100");
        let mut page_url = url.clone();
        let mut tokens = HashSet::new();
        let mut out = Vec::new();
        for _ in 0..1000 {
            let bytes = self.request("GET", &page_url, "", None)?;
            let page: Page<T> = serde_json::from_slice(&bytes).context("Read Google Tasks page")?;
            ensure!(page.kind == kind, "Unexpected Google Tasks response");
            out.extend(page.items);
            let Some(token) = page.next_page_token.filter(|token| !token.is_empty()) else {
                return Ok(out);
            };
            ensure!(
                tokens.insert(token.clone()),
                "Google Tasks pagination stopped making progress"
            );
            page_url = url.clone();
            page_url.query_pairs_mut().append_pair("pageToken", &token);
        }
        bail!("Google Tasks returned too many pages")
    }

    pub fn create(&self, list: &str, task: &Task) -> Result<Item> {
        let url = self.url(&["lists", list, "tasks"])?;
        Ok(serde_json::from_slice(&self.request(
            "POST",
            &url,
            "",
            Some(&payload(task)),
        )?)?)
    }

    pub fn update(&self, list: &str, id: &str, etag: &str, task: &Task) -> Result<Item> {
        let url = self.url(&["lists", list, "tasks", id])?;
        Ok(serde_json::from_slice(&self.request(
            "PATCH",
            &url,
            etag,
            Some(&payload(task)),
        )?)?)
    }

    pub fn delete(&self, list: &str, id: &str, etag: &str) -> Result<()> {
        let url = self.url(&["lists", list, "tasks", id])?;
        match self.request("DELETE", &url, etag, None) {
            Ok(_) => Ok(()),
            Err(err) if status(&err, 404) || status(&err, 410) => Ok(()),
            Err(err) => Err(err),
        }
    }
}

pub(super) fn status(err: &anyhow::Error, code: u16) -> bool {
    err.downcast_ref::<HttpStatus>()
        .is_some_and(|status| status.0 == code)
}
