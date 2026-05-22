use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

use naite_core::{
    CheckoutPullRequestOptions, CreatePullRequestOptions, HostingProvider, ListPullRequestsOptions,
    PullRequestSummary, Repository,
};
use reqwest::Client;
use tokio::sync::Semaphore;

use crate::persistence;

// Cap on concurrent avatar HTTP fetches. Without a cap, opening a large
// repo fires hundreds of simultaneous TLS handshakes against github.com,
// which is slower than letting a smaller pool pipeline keep-alive
// connections.
const AVATAR_FETCH_CONCURRENCY: usize = 12;
const AVATAR_FETCH_TIMEOUT: Duration = Duration::from_secs(5);

fn avatar_http_client() -> &'static Client {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        Client::builder()
            .user_agent(concat!("naite/", env!("CARGO_PKG_VERSION")))
            .timeout(AVATAR_FETCH_TIMEOUT)
            .build()
            .unwrap_or_else(|_| Client::new())
    })
}

fn avatar_fetch_semaphore() -> &'static Semaphore {
    static SEM: OnceLock<Semaphore> = OnceLock::new();
    SEM.get_or_init(|| Semaphore::new(AVATAR_FETCH_CONCURRENCY))
}

pub(crate) async fn list(
    path: PathBuf,
    options: ListPullRequestsOptions,
) -> Result<Vec<PullRequestSummary>, String> {
    tokio::task::spawn_blocking(move || -> Result<_, String> {
        let repo = Repository::open(&path).map_err(|e| e.to_string())?;
        repo.list_pull_requests(HostingProvider::GitHub, options)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("worker join error: {e}"))?
}

pub(crate) async fn create(
    path: PathBuf,
    options: CreatePullRequestOptions,
) -> Result<String, String> {
    tokio::task::spawn_blocking(move || -> Result<_, String> {
        let repo = Repository::open(&path).map_err(|e| e.to_string())?;
        repo.create_pull_request(HostingProvider::GitHub, options)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("worker join error: {e}"))?
}

pub(crate) async fn checkout(
    path: PathBuf,
    number: u32,
    options: CheckoutPullRequestOptions,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || -> Result<_, String> {
        let repo = Repository::open(&path).map_err(|e| e.to_string())?;
        repo.checkout_pull_request(HostingProvider::GitHub, number, options)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("worker join error: {e}"))?
}

pub(crate) async fn open_in_browser(path: PathBuf, number: u32) -> Result<(), String> {
    tokio::task::spawn_blocking(move || -> Result<_, String> {
        let repo = Repository::open(&path).map_err(|e| e.to_string())?;
        repo.open_pull_request_in_browser(HostingProvider::GitHub, number)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("worker join error: {e}"))?
}

/// Fetch an avatar URL into raw bytes. Returns the originating URL alongside
/// the result so the message handler can route the response into the
/// `AvatarCache` keyed by URL (the future itself is decoupled from caller
/// state by the time it resolves).
///
/// Order of operations:
/// 1. Read from the on-disk cache (instant, no network).
/// 2. Acquire a permit from the shared semaphore to bound concurrent fetches.
/// 3. Issue the request through a singleton `reqwest::Client` so keep-alive,
///    TLS pool, UA, and timeout are reused across calls.
/// 4. Write the fresh bytes back to the disk cache so the next launch skips
///    the network entirely.
pub(crate) async fn fetch_avatar(url: String) -> (String, Result<Vec<u8>, String>) {
    let cache_lookup_url = url.clone();
    let cached =
        tokio::task::spawn_blocking(move || persistence::load_avatar_bytes(&cache_lookup_url))
            .await
            .ok()
            .flatten();
    if let Some(bytes) = cached {
        return (url, Ok(bytes));
    }

    let _permit = avatar_fetch_semaphore().acquire().await.ok();

    let result = async {
        let response = avatar_http_client()
            .get(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !response.status().is_success() {
            return Err(format!("HTTP {}", response.status()));
        }
        let bytes = response.bytes().await.map_err(|e| e.to_string())?;
        Ok(bytes.to_vec())
    }
    .await;

    if let Ok(ref bytes) = result {
        let cache_write_url = url.clone();
        let cache_write_bytes = bytes.clone();
        tokio::task::spawn_blocking(move || {
            let _ = persistence::save_avatar_bytes(&cache_write_url, &cache_write_bytes);
        });
    }

    (url, result)
}
