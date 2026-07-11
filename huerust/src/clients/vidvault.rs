//! VidVault Client — MovieBox proxy with MKV download mirrors
//!
//! API flow:
//!   1. GET vidvault.ru/api/get-token → { t, e }
//!   2. POST vidvault.ru/api/download-proxy (x-request-token header)
//!      body: { type: "movie"|"tv", tmdbId: "..." }
//!   3. Response contains mp4Data (streams + captions) + mkvV3Data (direct MKV links)

use anyhow::{anyhow, Result};
use reqwest::header::HeaderValue;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const API_BASE: &str = "https://vidvault.ru/api";

#[derive(Debug, Deserialize)]
struct TokenResponse {
    t: String,
    #[allow(dead_code)]
    e: u64,
}

#[derive(Debug, Deserialize)]
struct ProxyResponse {
    #[serde(rename = "mp4Data")]
    mp4_data: Option<Mp4Data>,
    #[serde(rename = "mkvData")]
    mkv_data: Option<MkvData>,
    #[serde(rename = "mkvV2Data")]
    mkv_v2_data: Option<MkvData>,
    #[serde(rename = "mkvV3Data")]
    mkv_v3_data: Option<MkvV3Data>,
}

#[derive(Debug, Deserialize)]
struct MkvData {
    #[serde(default)]
    files: Vec<MkvFile>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct MkvFile {
    url: String,
    size: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct DownloadFile {
    pub url: String,
    pub size: String,
    pub format: String,
    pub quality: String,
}

#[derive(Debug, Serialize, Clone, Default)]
pub struct DownloadLinks {
    pub files: Vec<DownloadFile>,
    pub captions: Vec<Caption>,
}

#[derive(Debug, Deserialize)]
struct Mp4Data {
    #[serde(rename = "downloadInfo")]
    download_info: Option<DownloadInfoWrapper>,
}

#[derive(Debug, Deserialize)]
struct DownloadInfoWrapper {
    data: Option<DownloadInfo>,
}

#[derive(Debug, Deserialize)]
struct DownloadInfo {
    downloads: Option<Vec<Mp4Download>>,
    captions: Option<Vec<Caption>>,
}

#[derive(Debug, Deserialize)]
struct Mp4Download {
    url: String,
    resolution: i32,
    #[serde(default)]
    size: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Caption {
    pub lan: String,
    #[serde(rename = "lanName")]
    pub lan_name: String,
    pub url: String,
}

#[derive(Debug, Deserialize)]
struct MkvV3Data {
    #[serde(default)]
    downloads: Vec<MkvDownload>,
}

#[derive(Debug, Deserialize)]
struct MkvDownload {
    quality: String,
    #[serde(rename = "size")]
    _size: String,
    url: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct VidVaultStream {
    pub url: String,
    pub title: String,
    pub quality: String,
    pub format: String,
    pub provider: String,
    pub headers: Option<HashMap<String, String>>,
    pub subtitles: Option<Vec<Caption>>,
}

#[derive(Clone)]
pub struct VidVaultClient {
    client: reqwest::Client,
}

impl VidVaultClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(20))
                .connect_timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap(),
        }
    }

    async fn get_token(&self) -> Result<String> {
        let resp = self
            .client
            .get(format!("{}/get-token", API_BASE))
            .header("Accept", "application/json")
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!(
                "VidVault token endpoint returned {}",
                resp.status()
            ));
        }

        let data: TokenResponse = resp.json().await?;
        Ok(data.t)
    }

    pub async fn get_downloads(
        &self,
        tmdb_id: &str,
        media_type: &str,
        _season: u32,
        _episode: u32,
    ) -> Result<Vec<VidVaultStream>> {
        let token = self.get_token().await?;
        let mtype = if media_type == "movie" { "movie" } else { "tv" };

        let mut payload = serde_json::json!({
            "type": mtype,
            "tmdbId": tmdb_id,
        });

        // For TV, the API likely needs season/episode in the body
        if mtype == "tv" {
            payload["season"] = _season.to_string().into();
            payload["episode"] = _episode.to_string().into();
        }

        let resp = self
            .client
            .post(format!("{}/download-proxy", API_BASE))
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .header("x-request-token", HeaderValue::from_str(&token)?)
            .header("Origin", "https://vidvault.ru")
            .header("Referer", "https://vidvault.ru/")
            .json(&payload)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!(
                "VidVault download-proxy returned {}",
                resp.status()
            ));
        }

        let data: ProxyResponse = resp.json().await?;
        let mut results = Vec::new();

        let referer = "https://vidvault.ru/";
        let headers = {
            let mut h = HashMap::new();
            h.insert("Referer".to_string(), referer.to_string());
            h.insert("Origin".to_string(), "https://vidvault.ru".to_string());
            h
        };

        // Extract MP4 streams
        if let Some(mp4) = data.mp4_data {
            if let Some(info) = mp4.download_info {
                if let Some(downloads) = info.data.as_ref().and_then(|d| d.downloads.as_ref()) {
                    for d in downloads {
                        results.push(VidVaultStream {
                            url: d.url.clone(),
                            title: format!("Vault - {}p (MP4)", d.resolution),
                            quality: format!("{}p", d.resolution),
                            format: "mp4".to_string(),
                            provider: "Vault".to_string(),
                            headers: Some(headers.clone()),
                            subtitles: info.data.as_ref().and_then(|d| d.captions.clone()),
                        });
                    }
                }
            }
        }

        // Extract MKV download links
        if let Some(mkv) = data.mkv_v3_data {
            for d in mkv.downloads {
                results.push(VidVaultStream {
                    url: d.url.clone(),
                    title: format!("Vault - {} (MKV Download)", d.quality),
                    quality: d.quality.clone(),
                    format: "mkv".to_string(),
                    provider: "Vault".to_string(),
                    headers: Some(headers.clone()),
                    subtitles: None,
                });
            }
        }

        Ok(results)
    }

    pub async fn get_download_files(
        &self,
        tmdb_id: &str,
        media_type: &str,
        season: u32,
        episode: u32,
    ) -> Result<DownloadLinks> {
        let token = self.get_token().await?;
        let mtype = if media_type == "movie" { "movie" } else { "tv" };

        let mut payload = serde_json::json!({
            "type": mtype,
            "tmdbId": tmdb_id,
        });
        if mtype == "tv" {
            payload["season"] = season.into();
            payload["episode"] = episode.into();
        }

        let resp = self
            .client
            .post(format!("{}/download-proxy", API_BASE))
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .header("x-request-token", HeaderValue::from_str(&token)?)
            .header("Origin", "https://vidvault.ru")
            .header("Referer", "https://vidvault.ru/")
            .json(&payload)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!(
                "VidVault download-proxy returned {}",
                resp.status()
            ));
        }

        let data: ProxyResponse = resp.json().await?;
        let mut files: Vec<DownloadFile> = Vec::new();

        for src in [data.mkv_data, data.mkv_v2_data].into_iter().flatten() {
            for f in src.files {
                files.push(DownloadFile {
                    url: f.url,
                    size: f.size,
                    format: "mkv".to_string(),
                    quality: "MKV".to_string(),
                });
            }
        }

        if let Some(mkv3) = data.mkv_v3_data {
            for d in mkv3.downloads {
                files.push(DownloadFile {
                    url: d.url,
                    size: d._size,
                    format: "mkv".to_string(),
                    quality: d.quality.clone(),
                });
            }
        }

        let mut captions = Vec::new();
        if let Some(mp4) = data.mp4_data {
            if let Some(info) = mp4.download_info.and_then(|w| w.data) {
                for d in info.downloads.unwrap_or_default() {
                    let size_bytes = d.size.parse::<u64>().unwrap_or(0);
                    let size_str = if size_bytes > 0 {
                        format!("{:.2} MB", size_bytes as f64 / 1_048_576.0)
                    } else {
                        String::new()
                    };
                    files.push(DownloadFile {
                        url: d.url,
                        size: size_str,
                        format: "mp4".to_string(),
                        quality: format!("{}p", d.resolution),
                    });
                }
                captions = info.captions.unwrap_or_default();
            }
        }

        Ok(DownloadLinks { files, captions })
    }
}
