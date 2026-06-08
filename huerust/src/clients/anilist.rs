use anyhow::{anyhow, Result};
use reqwest_middleware::ClientWithMiddleware;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::models::anime::{AnimeCard, AnimeDetail, AnimeSearchResponse, EpisodeSummary};

const ANILIST_API: &str = "https://graphql.anilist.co";

#[derive(Clone)]
pub struct AniListClient {
    client: ClientWithMiddleware,
}

impl AniListClient {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .connect_timeout(std::time::Duration::from_secs(8))
            .build()
            .unwrap();
        Self {
            client: crate::http_client::wrap_with_retry(client),
        }
    }

    pub async fn search(&self, keyword: &str, page: u32, per_page: u32) -> Result<AnimeSearchResponse> {
        let query = r#"
query SearchAnime($search: String, $page: Int, $perPage: Int) {
  Page(page: $page, perPage: $perPage) {
    pageInfo { total perPage currentPage lastPage hasNextPage }
    media(search: $search, type: ANIME, sort: POPULARITY_DESC) {
      id
      title { english romaji native }
      description
      coverImage { large medium }
      bannerImage
      format
      status
      averageScore
      episodes
      duration
      genres
      seasonYear
      season
      startDate { year month day }
    }
  }
}
"#;

        let resp = self.client
            .post(ANILIST_API)
            .json(&json!({
                "query": query,
                "variables": {
                    "search": keyword,
                    "page": page,
                    "perPage": per_page
                }
            }))
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("AniList returned {}", resp.status()));
        }

        let data: serde_json::Value = resp.json().await?;
        let page_json = data.get("data").and_then(|d| d.get("Page")).unwrap_or(&serde_json::Value::Null);
        let empty_media: Vec<serde_json::Value> = vec![];
        let media = page_json.get("media").and_then(|m| m.as_array()).unwrap_or(&empty_media);

        let results: Vec<AnimeCard> = media.iter().filter_map(|m| {
            let id = m.get("id")?.as_i64()?.to_string();
            let title_obj = m.get("title")?;
            let title = title_obj.get("english")
                .and_then(|t| t.as_str())
                .or_else(|| title_obj.get("romaji").and_then(|t| t.as_str()))
                .unwrap_or("Unknown")
                .to_string();
            let japanese = title_obj.get("native").and_then(|t| t.as_str()).map(|s| s.to_string());
            let cover = m.get("coverImage").and_then(|c| c.get("large")).and_then(|u| u.as_str()).map(|s| s.to_string());
            let banner = m.get("bannerImage").and_then(|u| u.as_str()).map(|s| s.to_string());
            let synopsis = m.get("description").and_then(|d| d.as_str()).map(|s| s.to_string());
            let anime_type = m.get("format").and_then(|f| f.as_str()).map(|s| s.to_string());
            let status = m.get("status").and_then(|s| s.as_str()).map(|s| s.to_string());
            let score = m.get("averageScore").and_then(|s| s.as_i64()).map(|n| (n as f64 / 10.0).to_string());
            let total_eps = m.get("episodes").and_then(|e| e.as_u64()).map(|n| n as u32);
            let genres = m.get("genres").and_then(|g| g.as_array()).map(|arr| {
                arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()
            });
            let mal_id = Some(m.get("id")?.as_i64()? as u32);
            let duration = m.get("duration").and_then(|d| d.as_u64()).map(|n| format!("{} min", n));
            let year = m.get("seasonYear").and_then(|y| y.as_i64()).map(|y| y.to_string());

            Some(AnimeCard {
                id,
                title,
                japanese_title: japanese,
                synopsis,
                image: cover,
                banner_image: banner,
                anime_type,
                status,
                score,
                rating: None,
                genres,
                total_episodes: total_eps,
                total_subbed: None,
                total_dubbed: None,
                slug: mal_id.map(|id| format!("mal-{}", id)),
                mal_id,
                view_count: None,
                aired: year,
                duration,
                has_sub: false,
                has_dub: false,
            })
        }).collect();

        let total = page_json.get("pageInfo").and_then(|p| p.get("total")).and_then(|t| t.as_u64()).unwrap_or(0) as usize;

        Ok(AnimeSearchResponse {
            query: keyword.to_string(),
            total,
            page,
            limit: per_page,
            results,
        })
    }

    pub async fn get_detail(&self, id: &str) -> Result<AnimeDetail> {
        let anilist_id = id.trim_start_matches("mal-").parse::<i32>()?;

        let query = r#"
query AnimeDetail($id: Int) {
  Media(id: $id, type: ANIME) {
    id
    title { english romaji native }
    description
    coverImage { large medium }
    bannerImage
    format
    status
    averageScore
    episodes
    duration
    genres
    seasonYear
    season
    startDate { year month day }
    endDate { year month day }
    studios { nodes { name } }
    source
    rankings { rank type }
    trailer { id site }
    nextAiringEpisode { airingAt episode }
  }
}
"#;

        let resp = self.client
            .post(ANILIST_API)
            .json(&json!({
                "query": query,
                "variables": { "id": anilist_id }
            }))
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("AniList returned {}", resp.status()));
        }

        let data: serde_json::Value = resp.json().await?;
        let media = data.get("data").and_then(|d| d.get("Media")).ok_or_else(|| anyhow!("Anime not found"))?;

        let title_obj = media.get("title").unwrap_or(&serde_json::Value::Null);
        let title = title_obj.get("english")
            .and_then(|t| t.as_str())
            .or_else(|| title_obj.get("romaji").and_then(|t| t.as_str()))
            .unwrap_or("Unknown")
            .to_string();
        let japanese = title_obj.get("native").and_then(|t| t.as_str()).map(|s| s.to_string());
        let cover = media.get("coverImage").and_then(|c| c.get("large")).and_then(|u| u.as_str()).map(|s| s.to_string());
        let banner = media.get("bannerImage").and_then(|u| u.as_str()).map(|s| s.to_string());
        let synopsis = media.get("description").and_then(|d| d.as_str()).map(|s| s.to_string());
        let anime_type = media.get("format").and_then(|f| f.as_str()).map(|s| s.to_string());
        let status = media.get("status").and_then(|s| s.as_str()).map(|s| s.to_string());
        let score = media.get("averageScore").and_then(|s| s.as_i64()).map(|n| (n as f64 / 10.0).to_string());
        let total_eps = media.get("episodes").and_then(|e| e.as_u64()).map(|n| n as u32);
        let genres = media.get("genres").and_then(|g| g.as_array()).map(|arr| {
            arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()
        });
        let mal_id = media.get("id").and_then(|i| i.as_i64()).map(|i| i as u32);
        let duration = media.get("duration").and_then(|d| d.as_u64()).map(|n| format!("{} min", n));
        let year = media.get("seasonYear").and_then(|y| y.as_i64()).map(|y| y.to_string());

        let studios = media.get("studios").and_then(|s| s.get("nodes")).and_then(|n| n.as_array()).map(|arr| {
            arr.iter().filter_map(|n| n.get("name").and_then(|s| s.as_str())).collect::<Vec<_>>().join(", ")
        });

        let source = media.get("source").and_then(|s| s.as_str()).map(|s| s.to_string());

        let episode_count = media.get("episodes").and_then(|e| e.as_u64()).unwrap_or(0) as u32;
        let episodes: Option<Vec<EpisodeSummary>> = if episode_count > 0 {
            Some((1..=episode_count).map(|n| EpisodeSummary {
                id: format!("{}-{}", id, n),
                number: n,
                title: None,
                sub_count: 0,
                dub_count: 0,
                slug: Some(format!("{}-{}", id, n)),
                slugs: None,
            }).collect())
        } else {
            None
        };

        let card = AnimeCard {
            id: id.to_string(),
            title,
            japanese_title: japanese,
            synopsis,
            image: cover,
            banner_image: banner,
            anime_type,
            status,
            score,
            rating: None,
            genres,
            total_episodes: total_eps,
            total_subbed: None,
            total_dubbed: None,
            slug: mal_id.map(|id| format!("mal-{}", id)),
            mal_id,
            view_count: None,
            aired: year,
            duration,
            has_sub: false,
            has_dub: false,
        };

        Ok(AnimeDetail {
            card,
            broadcast: None,
            premiered: None,
            producers: studios,
            licensors: None,
            source,
            members: None,
            favorites: None,
            popularity: None,
            ranked: None,
            episodes,
        })
    }
}
