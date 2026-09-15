//! A thin client for Hugging Face Hub's public model-listing API.
//!
//! Nothing here is a hardcoded list of models — every result, count, and
//! piece of metadata comes from a live API call. This crate only knows how
//! to ask Hugging Face's own endpoints and shape the answer; it has no
//! opinion about which models exist.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::Hash;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

const API_BASE: &str = "https://huggingface.co/api/models";

struct CacheEntry<V> {
    value: V,
    fetched_at: Instant,
}

// Process-lifetime, in-memory caches for the three Hugging Face Hub calls
// this crate makes. No persistence across app restarts — the app only asks
// to avoid re-hitting the network for repeats within one running session,
// and `ttl` is a per-call parameter (not baked into the cache itself) so a
// live "cache duration" setting in the frontend takes effect immediately,
// without needing to push config changes into this crate at all.
type SearchKey = (String, String, usize);
static SEARCH_CACHE: LazyLock<Mutex<HashMap<SearchKey, CacheEntry<Vec<ModelSummary>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static DETAIL_CACHE: LazyLock<Mutex<HashMap<String, CacheEntry<Option<DetailFields>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static TREE_CACHE: LazyLock<Mutex<HashMap<String, CacheEntry<Vec<GgufFile>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

// ttl == Duration::ZERO means "caching off": never read a cached value, and
// never bother storing a fresh one either.
fn get_cached<K: Eq + Hash, V: Clone>(cache: &Mutex<HashMap<K, CacheEntry<V>>>, key: &K, ttl: Duration) -> Option<V> {
    if ttl.is_zero() {
        return None;
    }
    let map = cache.lock().unwrap();
    map.get(key).filter(|entry| entry.fetched_at.elapsed() < ttl).map(|entry| entry.value.clone())
}

fn put_cached<K: Eq + Hash, V>(cache: &Mutex<HashMap<K, CacheEntry<V>>>, key: K, value: V, ttl: Duration) {
    if ttl.is_zero() {
        return;
    }
    cache.lock().unwrap().insert(key, CacheEntry { value, fetched_at: Instant::now() });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortBy {
    Downloads,
    Likes,
    Trending,
    RecentlyUpdated,
}

impl SortBy {
    fn as_query_value(self) -> &'static str {
        match self {
            SortBy::Downloads => "downloads",
            SortBy::Likes => "likes",
            SortBy::Trending => "trendingScore",
            SortBy::RecentlyUpdated => "lastModified",
        }
    }
}

/// One search result, enriched with the per-model detail fields (parameter
/// count, architecture, context length) the search endpoint itself doesn't
/// carry — fetched concurrently so browsing still feels immediate.
#[derive(Debug, Clone, Serialize)]
pub struct ModelSummary {
    pub id: String,
    pub pipeline_tag: Option<String>,
    pub downloads: u64,
    pub likes: u64,
    pub last_modified: Option<String>,
    pub gguf_file_count: usize,
    /// `None` when Hugging Face's own GGUF metadata extraction didn't run
    /// for this repo (e.g. very new or unusually-structured uploads) — an
    /// honest gap, not a guess.
    pub parameter_count: Option<u64>,
    pub architecture: Option<String>,
    pub context_length: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawModel {
    id: String,
    #[serde(default)]
    pipeline_tag: Option<String>,
    #[serde(default)]
    downloads: u64,
    #[serde(default)]
    likes: u64,
    #[serde(default, rename = "lastModified")]
    last_modified: Option<String>,
    #[serde(default)]
    siblings: Vec<RawSibling>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawSibling {
    rfilename: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RawModelDetail {
    #[serde(default)]
    gguf: Option<RawGgufInfo>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawGgufInfo {
    #[serde(default)]
    total: Option<u64>,
    #[serde(default)]
    architecture: Option<String>,
    #[serde(default)]
    context_length: Option<u64>,
}

/// Searches Hugging Face Hub for GGUF, text-generation models — the only
/// kind this app can actually load — sorted by `sort`, and enriches each
/// result with its real parameter count/architecture/context length so
/// that's visible while browsing, not just after picking one.
pub async fn search_models(query: &str, sort: SortBy, limit: usize, cache_ttl: Duration) -> Result<Vec<ModelSummary>> {
    let cache_key = (query.to_string(), sort.as_query_value().to_string(), limit);
    if let Some(cached) = get_cached(&SEARCH_CACHE, &cache_key, cache_ttl) {
        return Ok(cached);
    }

    let client = reqwest::Client::builder().build().context("failed to build HTTP client")?;

    let response = client
        .get(API_BASE)
        .query(&[
            ("search", query),
            ("filter", "gguf"),
            ("filter", "text-generation"),
            ("sort", sort.as_query_value()),
            ("direction", "-1"),
            ("limit", &limit.to_string()),
            ("full", "true"),
        ])
        .send()
        .await
        .context("failed to reach Hugging Face Hub")?
        .error_for_status()
        .context("Hugging Face Hub returned an error")?;

    let raw: Vec<RawModel> = response.json().await.context("failed to parse Hugging Face Hub response")?;

    let mut details = tokio::task::JoinSet::new();
    for model in &raw {
        let client = client.clone();
        let id = model.id.clone();
        details.spawn(async move { (id.clone(), fetch_detail(&client, &id, cache_ttl).await) });
    }
    let mut detail_by_id = std::collections::HashMap::new();
    while let Some(joined) = details.join_next().await {
        if let Ok((id, detail)) = joined {
            detail_by_id.insert(id, detail);
        }
    }

    let result: Vec<ModelSummary> = raw
        .into_iter()
        .map(|m| {
            let detail = detail_by_id.get(&m.id).cloned().flatten();
            ModelSummary {
                gguf_file_count: m.siblings.iter().filter(|s| s.rfilename.ends_with(".gguf")).count(),
                id: m.id,
                pipeline_tag: m.pipeline_tag,
                downloads: m.downloads,
                likes: m.likes,
                last_modified: m.last_modified,
                parameter_count: detail.as_ref().and_then(|d| d.total),
                architecture: detail.as_ref().and_then(|d| d.architecture.clone()),
                context_length: detail.as_ref().and_then(|d| d.context_length),
            }
        })
        .collect();

    put_cached(&SEARCH_CACHE, cache_key, result.clone(), cache_ttl);
    Ok(result)
}

#[derive(Debug, Clone)]
struct DetailFields {
    total: Option<u64>,
    architecture: Option<String>,
    context_length: Option<u64>,
}

/// Best-effort: a single repo's detail call failing (rate limit, transient
/// network issue, unusual repo) drops that row's extra fields rather than
/// failing the whole search.
async fn fetch_detail(client: &reqwest::Client, repo_id: &str, cache_ttl: Duration) -> Option<DetailFields> {
    if let Some(cached) = get_cached(&DETAIL_CACHE, &repo_id.to_string(), cache_ttl) {
        return cached;
    }
    let url = format!("{API_BASE}/{repo_id}");
    let result = (|| async {
        let response = client.get(&url).send().await.ok()?.error_for_status().ok()?;
        let raw: RawModelDetail = response.json().await.ok()?;
        let gguf = raw.gguf?;
        Some(DetailFields { total: gguf.total, architecture: gguf.architecture, context_length: gguf.context_length })
    })()
    .await;
    put_cached(&DETAIL_CACHE, repo_id.to_string(), result.clone(), cache_ttl);
    result
}

#[derive(Debug, Clone, Serialize)]
pub struct GgufFile {
    pub filename: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Deserialize)]
struct RawTreeEntry {
    #[serde(rename = "type")]
    kind: String,
    path: String,
    #[serde(default)]
    size: u64,
}

/// Every GGUF file in a repo, with its real size — the picker for "which
/// quantization" once a model's been chosen.
pub async fn list_gguf_files(repo_id: &str, cache_ttl: Duration) -> Result<Vec<GgufFile>> {
    if let Some(cached) = get_cached(&TREE_CACHE, &repo_id.to_string(), cache_ttl) {
        return Ok(cached);
    }

    let url = format!("{API_BASE}/{repo_id}/tree/main");
    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .send()
        .await
        .context("failed to reach Hugging Face Hub")?
        .error_for_status()
        .context("Hugging Face Hub returned an error")?;
    let entries: Vec<RawTreeEntry> = response.json().await.context("failed to parse file listing")?;

    let result: Vec<GgufFile> = entries
        .into_iter()
        .filter(|e| e.kind == "file" && e.path.ends_with(".gguf"))
        .map(|e| GgufFile { filename: e.path, size_bytes: e.size })
        .collect();

    put_cached(&TREE_CACHE, repo_id.to_string(), result.clone(), cache_ttl);
    Ok(result)
}

/// Resolves a (model_dir, gguf_file) pair the frontend hands us to a real
/// local file path. `model_dir` means two different things depending on how
/// the model was picked: a real local directory (the manual/file-picker
/// path), or a Hugging Face repo id (the Browse-models path) — mistral.rs's
/// own loader already handles that ambiguity internally via its bundled
/// `hf-hub` client, but nothing else that reads the GGUF file directly (GGUF
/// inspection, embedding sampling) ever learns which case it's in unless it
/// checks too. This does the same local-then-cached-download resolution.
pub fn resolve_gguf_path(model_dir: &str, gguf_file: &str) -> Result<PathBuf> {
    let local = PathBuf::from(model_dir).join(gguf_file);
    if local.exists() {
        return Ok(local);
    }
    hf_hub::Cache::default().model(model_dir.to_string()).get(gguf_file).with_context(|| {
        format!("'{gguf_file}' isn't available locally and no cached download was found for '{model_dir}'")
    })
}
