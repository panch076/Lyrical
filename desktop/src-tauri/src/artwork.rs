//! Resolves album artwork to a data URL the webviews can display directly.
//! Order: the player's own artwork, then the iTunes Search API.

use crate::media::Artwork;
use base64::Engine;
use regex::Regex;
use serde::Deserialize;
use std::sync::LazyLock;

static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .user_agent("Lyrical/1.0")
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .expect("http client")
});

pub async fn resolve(source: Option<Artwork>, title: &str, artist: &str) -> Option<String> {
    let bytes = match source {
        Some(Artwork::Bytes(bytes)) => Some(bytes),
        Some(Artwork::Url(url)) => load(&url).await,
        None => None,
    };
    let bytes = match bytes {
        Some(bytes) => bytes,
        None => {
            let url = itunes_artwork(title, artist).await?;
            download(&url).await?
        }
    };
    Some(data_url(&bytes))
}

async fn load(url: &str) -> Option<Vec<u8>> {
    if url.starts_with("file://") {
        let path = url::Url::parse(url).ok()?.to_file_path().ok()?;
        return std::fs::read(path).ok();
    }
    if url.starts_with("http") {
        if let Some(bytes) = download(&high_resolution(url)).await {
            return Some(bytes);
        }
        return download(url).await;
    }
    None
}

async fn download(url: &str) -> Option<Vec<u8>> {
    let response = CLIENT.get(url).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    let bytes = response.bytes().await.ok()?;
    (bytes.len() > 64).then(|| bytes.to_vec())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Item {
    artwork_url100: Option<String>,
}

#[derive(Deserialize)]
struct Response {
    results: Vec<Item>,
}

async fn itunes_artwork(title: &str, artist: &str) -> Option<String> {
    let term = format!("{artist} {title}");
    let response: Response = CLIENT
        .get("https://itunes.apple.com/search")
        .query(&[("term", term.as_str()), ("entity", "song"), ("limit", "1")])
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;
    let url = response.results.into_iter().find_map(|item| item.artwork_url100)?;
    Some(high_resolution(&url))
}

static APPLE_SIZE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d+x\d+(bb|cc|sr)?").unwrap());
static GOOGLE_SIZE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"=w\d+-h\d+").unwrap());

/// Rewrites CDN artwork URLs to a large size (Apple's …/100x100bb.jpg and
/// Google's/YouTube Music's …=w120-h120-l90-rj).
pub fn high_resolution(url: &str) -> String {
    if url.contains("mzstatic.com") {
        APPLE_SIZE.replace_all(url, "1200x1200bb").into_owned()
    } else if url.contains("googleusercontent.com") || url.contains("ggpht.com") {
        GOOGLE_SIZE.replace_all(url, "=w1200-h1200").into_owned()
    } else {
        url.to_string()
    }
}

fn data_url(bytes: &[u8]) -> String {
    let mime = if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        "image/png"
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        "image/webp"
    } else {
        "image/jpeg"
    };
    format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))
}
