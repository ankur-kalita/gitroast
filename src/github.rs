//! The only file that talks to the internet.

use crate::profile::{Repo, User};
use serde::de::DeserializeOwned;
use std::fmt;
use std::time::Duration;
use ureq::Agent;

const API: &str = "https://api.github.com";
/// 3 pages of 100 is plenty to judge someone, and keeps us well inside the rate limit.
const MAX_PAGES: u32 = 3;

#[derive(Debug)]
pub enum FetchError {
    NotFound(String),
    RateLimited,
    Other(String),
}

impl fmt::Display for FetchError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            FetchError::NotFound(name) => write!(f, "no GitHub user called '{name}'"),
            FetchError::RateLimited => write!(
                f,
                "GitHub rate limit hit (60 requests/hour without a token). \
                 Set GITHUB_TOKEN to raise it, or wait an hour."
            ),
            FetchError::Other(msg) => write!(f, "{msg}"),
        }
    }
}

pub struct Client {
    agent: Agent,
    token: Option<String>,
}

impl Client {
    /// Uses `GITHUB_TOKEN` from the environment if it is set.
    pub fn from_env() -> Self {
        let token = std::env::var("GITHUB_TOKEN").ok().filter(|t| !t.is_empty());
        // Without timeouts, a network with broken IPv6 hangs forever on the first address.
        let agent: Agent = Agent::config_builder()
            .timeout_connect(Some(Duration::from_secs(3)))
            .timeout_global(Some(Duration::from_secs(20)))
            .build()
            .into();
        Client { agent, token }
    }

    pub fn user(&self, name: &str) -> Result<User, FetchError> {
        self.get(&format!("{API}/users/{name}"), name)
    }

    pub fn repos(&self, name: &str) -> Result<Vec<Repo>, FetchError> {
        let mut all = Vec::new();
        for page in 1..=MAX_PAGES {
            let url = format!("{API}/users/{name}/repos?per_page=100&page={page}&sort=pushed");
            let batch: Vec<Repo> = self.get(&url, name)?;
            let last = batch.len() < 100;
            all.extend(batch);
            if last {
                break;
            }
        }
        Ok(all)
    }

    fn get<T: DeserializeOwned>(&self, url: &str, name: &str) -> Result<T, FetchError> {
        let mut request = self
            .agent
            .get(url)
            .header("User-Agent", "gitroast")
            .header("Accept", "application/vnd.github+json");
        if let Some(token) = &self.token {
            request = request.header("Authorization", &format!("Bearer {token}"));
        }
        match request.call() {
            Ok(mut response) => response
                .body_mut()
                .read_json()
                .map_err(|e| FetchError::Other(format!("GitHub sent something unexpected: {e}"))),
            Err(ureq::Error::StatusCode(404)) => Err(FetchError::NotFound(name.to_string())),
            Err(ureq::Error::StatusCode(403 | 429)) => Err(FetchError::RateLimited),
            Err(e) => Err(FetchError::Other(format!("could not reach GitHub: {e}"))),
        }
    }
}
