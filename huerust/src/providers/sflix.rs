//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use crate::clients::moviebox::{quality_from_resolutions, StreamData};
use crate::models::{StreamResult, Subtitle};
use anyhow::Result;
use reqwest::{
    header::{HeaderMap, HeaderValue, CONTENT_TYPE, REFERER},
    Client,
};
use serde::Deserialize;
use std::collections::HashMap;
use tracing::{info, warn};

const BASE_URL: &str = "https://sflix.film";
const API_BASE: &str = "https://h5-api.aoneroom.com/wefeed-h5api-bff";
const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
const DEFAULT_COOKIE: &str = "sflix_token=%22eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJ1aWQiOjMxMTYwODUxODIwODQ5MjQ1MTIsImF0cCI6MywiZXh0IjoiMTc2ODA4NTkyNCIsImV4cCI6MTc3NTg2MTkyNCwiaWF0IjoxNzY4MDg1NjI0fQ.K6fuHWpTcLP0WjQx9ZFe6LSo_oNVNKiuC3MOA2HRw1s%22; account=2904345630854337384|0|H5|1768086238|; uuid=181f2237-fb79-4245-9a8a-2d5f0a60c4aa; sflix_i18n_lang=en";

pub struct SflixClient {
    client: Client,
}

#[derive(Debug, Deserialize)]
struct CaptionResponse {
    code: i32,
    data: Option<CaptionData>,
}

#[derive(Debug, Deserialize)]
struct CaptionData {
    captions: Option<Vec<SflixCaption>>,
}

#[derive(Debug, Deserialize)]
struct SflixCaption {
    url: Option<String>,
    lan: Option<String>,
    #[serde(rename = "lanName")]
    lan_name: Option<String>,
}

impl SflixClient {
    pub fn new() -> Self {
        // Warn once at startup if the fallback credential is expired
        let cookie =
            crate::user_config::HueUserConfig::get_live().sflix_cookie_or_env(DEFAULT_COOKIE);
        if cookie == DEFAULT_COOKIE && crate::security::jwt_is_expired(DEFAULT_COOKIE) {
            warn!("[SFlix] Default JWT is expired. Set sflix_cookie in user-config.json to restore functionality.");
        }
        Self {
            client: Client::builder()
                .user_agent(DEFAULT_USER_AGENT)
                .timeout(std::time::Duration::from_secs(30))
                .connect_timeout(std::time::Duration::from_secs(10))
                .tcp_keepalive(std::time::Duration::from_secs(60))
                .pool_max_idle_per_host(2)
                .pool_idle_timeout(std::time::Duration::from_secs(90))
                .build()
                .unwrap(),
        }
    }

    fn get_base_headers(&self) -> HeaderMap {
        let cookie =
            crate::user_config::HueUserConfig::get_live().sflix_cookie_or_env(DEFAULT_COOKIE);
        let mut headers = HeaderMap::new();
        headers.insert("Accept", HeaderValue::from_static("application/json"));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(REFERER, HeaderValue::from_static("https://sflix.film/"));
        if let Ok(v) = HeaderValue::from_str(&cookie) {
            headers.insert("Cookie", v);
        }
        headers
    }

    pub async fn search(&self, keyword: &str, media_type: &str) -> Result<Vec<serde_json::Value>> {
        let url = format!("{}/subject/search", API_BASE);
        let subject_type = if media_type == "tv" { 2 } else { 1 };

        let payload = serde_json::json!({
            "keyword": keyword,
            "page": "1",
            "perPage": 24,
            "subjectType": subject_type
        });

        let resp = self
            .client
            .post(&url)
            .headers(self.get_base_headers())
            .json(&payload)
            .send()
            .await?;

        let data: serde_json::Value = resp.json().await?;
        let items = data["data"]["items"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        Ok(items)
    }

    pub async fn get_streams(
        &self,
        slug: &str,
        subject_id: &str,
        media_type: &str,
        season: u32,
        episode: u32,
    ) -> Result<Option<StreamData>> {
        let se = season;
        let ep = episode;

        let (type_path, type_query) = if media_type == "tv" {
            ("series", "/series/detail")
        } else {
            ("movies", "/movie/detail")
        };

        let referer_url = format!(
            "{}/spa/videoPlayPage/{}/{}?id={}&type={}&lang=en",
            BASE_URL,
            type_path,
            slug,
            subject_id,
            urlencoding::encode(type_query)
        );

        let play_url = format!(
            "{}/wefeed-h5api-bff/subject/play?subjectId={}&se={}&ep={}&detailPath={}",
            BASE_URL, subject_id, se, ep, slug
        );

        let mut headers = self.get_base_headers();
        headers.insert(
            REFERER,
            HeaderValue::from_str(&referer_url)
                .unwrap_or_else(|_| HeaderValue::from_static(BASE_URL)),
        );

        let resp = self.client.get(&play_url).headers(headers).send().await?;

        let data: serde_json::Value = resp.json().await?;
        if data["code"] == 0 {
            let parsed: Option<StreamData> = serde_json::from_value(data["data"].clone()).ok();
            // Fallback to scraper if API returns empty streams
            if let Some(ref sd) = parsed {
                let has_streams = sd.streams.as_ref().map(|v| !v.is_empty()).unwrap_or(false);
                let has_hls = sd.hls.as_ref().map(|v| !v.is_empty()).unwrap_or(false);
                let has_dash = sd.dash.as_ref().map(|v| !v.is_empty()).unwrap_or(false);
                if !has_streams && !has_hls && !has_dash {
                    return Ok(None);
                }
            }
            return Ok(parsed);
        }

        Ok(None)
    }

    async fn get_captions(
        &self,
        subject_id: &str,
        detail_path: &str,
        stream_id: Option<&str>,
        format: &str,
    ) -> Result<Vec<Subtitle>> {
        let effective_id = stream_id.filter(|id| !id.is_empty()).unwrap_or(subject_id);

        if effective_id == subject_id {
            warn!(
                "[SFlix] stream_id missing for subject_id={}, using subject_id as fallback",
                subject_id
            );
        }

        let url = format!(
            "{}/subject/caption?format={}&id={}&subjectId={}&detailPath={}",
            API_BASE,
            urlencoding::encode(format),
            urlencoding::encode(effective_id),
            urlencoding::encode(subject_id),
            urlencoding::encode(detail_path),
        );

        let resp = self
            .client
            .get(&url)
            .headers(self.get_base_headers())
            .send()
            .await?;

        if !resp.status().is_success() {
            return Ok(vec![]);
        }

        let data = resp.json::<CaptionResponse>().await?;
        if data.code != 0 {
            return Ok(vec![]);
        }

        Ok(data
            .data
            .and_then(|d| d.captions)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|caption| {
                let url = caption.url?;
                if url.is_empty() {
                    return None;
                }
                let lang = caption.lan.unwrap_or_else(|| "en".to_string());
                let label = caption.lan_name.unwrap_or_else(|| lang.clone());
                Some(Subtitle { url, lang, label })
            })
            .collect())
    }

    pub async fn get_streams_normalized(
        &self,
        title: &str,
        media_type: &str,
        year: Option<u32>,
        season: u32,
        episode: u32,
    ) -> Result<Vec<StreamResult>> {
        info!(
            "[SFlix Rust] Searching for: \"{}\" ({}) S{} E{}",
            title, media_type, season, episode
        );

        let items = self.search(title, media_type).await?;
        if items.is_empty() {
            return Ok(vec![]);
        }

        // Debug: dump each candidate's title/year for diagnosis
        for (i, item) in items.iter().enumerate() {
            let ctitle = item["title"].as_str().unwrap_or_default();
            let cyear = extract_year(item);
            let score = title_score(title, ctitle);
            info!(
                "[SFlix Rust]   candidate[{}] title={:?} year={:?} score={:.1}",
                i, ctitle, cyear, score
            );
        }

        let best_match = match pick_best_match(&items, title, year) {
            Some(m) => m,
            None => {
                warn!("[SFlix Rust] No plausible match for \"{}\" (year={:?}) in {} results — skipping SFlix", title, year, items.len());
                return Ok(vec![]);
            }
        };
        let chosen_title = best_match["title"].as_str().unwrap_or_default();
        let chosen_year = extract_year(best_match);
        info!(
            "[SFlix Rust]   CHOSE title={:?} year={:?} for query {:?}",
            chosen_title, chosen_year, title
        );
        let slug = best_match["detailPath"].as_str().unwrap_or_default();
        let subject_id = best_match["subjectId"].as_str().unwrap_or_default();

        match self
            .get_streams(slug, subject_id, media_type, season, episode)
            .await?
        {
            Some(data) => {
                let mut results = Vec::new();
                let mut headers = HashMap::new();
                headers.insert("Referer".to_string(), format!("{}/", BASE_URL));
                headers.insert("User-Agent".to_string(), DEFAULT_USER_AGENT.to_string());

                if let Some(streams) = data.streams {
                    for s in streams {
                        let subtitles = self
                            .get_captions(subject_id, slug, s.id.as_deref(), "MP4")
                            .await
                            .unwrap_or_default();
                        let quality = s
                            .quality
                            .as_deref()
                            .map(str::to_string)
                            .filter(|q| !q.trim().is_empty() && q.to_lowercase() != "auto")
                            .unwrap_or_else(|| {
                                quality_from_resolutions(s.resolutions.as_deref())
                            });
                        results.push(StreamResult {
                            url: s.url,
                            title: format!("Nova - {} (MP4)", quality),
                            quality,
                            provider: "Nova".to_string(),
                            format: "mp4".to_string(),
                            subtitles: if subtitles.is_empty() {
                                None
                            } else {
                                Some(subtitles)
                            },
                            headers: Some(headers.clone()),
                            extra_metadata: None,
                        });
                    }
                }

                if let Some(hls) = data.hls {
                    for s in hls {
                        let subtitles = self
                            .get_captions(subject_id, slug, s.id.as_deref(), "HLS")
                            .await
                            .unwrap_or_default();
                        let quality = s
                            .quality
                            .as_deref()
                            .map(str::to_string)
                            .filter(|q| !q.trim().is_empty() && q.to_lowercase() != "auto")
                            .unwrap_or_else(|| {
                                quality_from_resolutions(s.resolutions.as_deref())
                            });
                        results.push(StreamResult {
                            url: s.url,
                            title: format!("Nova - {} (HLS)", quality),
                            quality,
                            provider: "Nova".to_string(),
                            format: "hls".to_string(),
                            subtitles: if subtitles.is_empty() {
                                None
                            } else {
                                Some(subtitles)
                            },
                            headers: Some(headers.clone()),
                            extra_metadata: None,
                        });
                    }
                }

                if let Some(dash) = data.dash {
                    for s in dash {
                        let subtitles = self
                            .get_captions(subject_id, slug, s.id.as_deref(), "DASH")
                            .await
                            .unwrap_or_default();
                        let quality = s
                            .quality
                            .as_deref()
                            .map(str::to_string)
                            .filter(|q| !q.trim().is_empty() && q.to_lowercase() != "auto")
                            .unwrap_or_else(|| {
                                quality_from_resolutions(s.resolutions.as_deref())
                            });
                        results.push(StreamResult {
                            url: s.url,
                            title: format!("Nova - {} (DASH)", quality),
                            quality,
                            provider: "Nova".to_string(),
                            format: "dash".to_string(),
                            subtitles: if subtitles.is_empty() {
                                None
                            } else {
                                Some(subtitles)
                            },
                            headers: Some(headers.clone()),
                            extra_metadata: None,
                        });
                    }
                }
                Ok(results)
            }
            None => {
                warn!(
                    "[SFlix Rust] API returned no streams for {} ({})",
                    slug, media_type
                );
                Ok(vec![])
            }
        }
    }
}

fn normalize(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn title_score(query: &str, candidate: &str) -> f32 {
    let q = normalize(query);
    let c = normalize(candidate);
    if q.is_empty() || c.is_empty() {
        return 0.0;
    }
    if c == q {
        return 100.0;
    }
    if c.contains(&q) || q.contains(&c) {
        return 80.0;
    }
    let q_words: Vec<&str> = q.split_whitespace().collect();
    let c_words: Vec<&str> = c.split_whitespace().collect();
    let q_set: std::collections::HashSet<&str> = q_words.iter().copied().collect();
    let c_set: std::collections::HashSet<&str> = c_words.iter().copied().collect();
    let overlap = q_set.intersection(&c_set).count() as f32;
    // Penalize candidates with significantly more words than the query —
    // "Spider-Man Noir" vs "Spider-Noir" is a 1-word extension that's
    // probably still a match, but "Off Campus" vs "Spider-Noir" is
    // completely different. Score by overlap on the *shorter* side so a
    // short query like "Spider-Noir" still gets full credit when one of
    // its words appears in the candidate.
    let short_len = q_words.len().min(c_words.len()) as f32;
    if short_len == 0.0 {
        return 0.0;
    }
    (overlap / short_len) * 60.0
}

fn extract_year(item: &serde_json::Value) -> Option<u32> {
    let release = item["releaseDate"]
        .as_str()
        .or_else(|| item["release_date"].as_str())
        .or_else(|| item["year"].as_str())?;
    release.get(..4)?.parse().ok()
}

fn pick_best_match<'a>(
    items: &'a [serde_json::Value],
    title: &str,
    year: Option<u32>,
) -> Option<&'a serde_json::Value> {
    // Reject if there's no plausible title match. Without a minimum title
    // similarity threshold, a year-only "match" can pick a totally unrelated
    // show (e.g. searching "Black Noir" returning "Off Campus" because the
    // years align but titles share zero words).
    const MIN_TITLE_SCORE: f32 = 25.0;

    let mut best: Option<(&'a serde_json::Value, f32)> = None;
    for item in items {
        let candidate_title = item["title"].as_str().unwrap_or_default();
        let t_score = title_score(title, candidate_title);
        if t_score < MIN_TITLE_SCORE {
            continue;
        }
        let mut score = t_score;
        if let (Some(target_year), Some(cand_year)) = (year, extract_year(item)) {
            let diff = (target_year as i32 - cand_year as i32).abs();
            // Year outside a ±2 window = hard reject, not a small penalty.
            // Otherwise a "Spider-Man Noir" (2018) match would beat nothing
            // when the user actually wants "Spider-Noir" (2026 TV show).
            if diff > 2 {
                continue;
            }
            let year_bonus = match diff {
                0 => 20.0,
                1 => 10.0,
                _ => 4.0,
            };
            score += year_bonus;
        }
        if best.map_or(true, |(_, s)| score > s) {
            best = Some((item, score));
        }
    }
    best.map(|(item, _)| item)
}
