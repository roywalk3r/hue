//! Hue Stream API — Sports Routes
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::{Datelike, NaiveDate, TimeZone, Utc};
use serde::Deserialize;
use serde_json::json;
use tracing::{info, warn};
use utoipa::ToSchema;
use validator::Validate;

use crate::app::AppState;
use crate::clients::aisports::SportsStream;
use crate::proxy::dynamic_proxy_base;
use crate::security::sign_proxy_url;

#[derive(Deserialize, ToSchema, Validate)]
pub struct SportsStreamQuery {
    #[validate(length(min = 1, max = 200))]
    pub q: String,
}

#[derive(Deserialize, ToSchema, Validate)]
pub struct LatestMatchesQuery {
    #[serde(default = "default_match_type", alias = "matchType")]
    #[validate(length(min = 1, max = 50))]
    pub match_type: String,
    #[serde(default = "default_latest_count")]
    #[validate(range(min = 1, max = 50))]
    pub count: u32,
}

#[derive(Deserialize, ToSchema, Validate)]
pub struct LeagueMatchesQuery {
    #[serde(default = "default_league_id", alias = "leagueId")]
    #[validate(length(min = 1, max = 32))]
    pub league_id: String,
}

#[derive(Deserialize, ToSchema, Validate)]
pub struct GroupedMatchesQuery {
    #[serde(default)]
    pub status: i32,
    #[serde(default = "default_sport_type", alias = "matchType")]
    #[validate(length(min = 1, max = 50))]
    pub match_type: String,
    #[serde(default, alias = "startTime")]
    #[validate(length(max = 20))]
    pub start_time: String,
    #[serde(default, alias = "endTime")]
    #[validate(length(max = 20))]
    pub end_time: String,
    #[serde(default)]
    #[validate(length(max = 10))]
    pub date: String,
}

#[derive(Deserialize, ToSchema, Validate)]
pub struct MatchFilterTabsQuery {
    #[serde(default = "default_sport_type", alias = "matchType")]
    #[validate(length(min = 1, max = 50))]
    pub match_type: String,
    #[serde(default, alias = "startTime")]
    #[validate(length(max = 20))]
    pub start_time: String,
    #[serde(default, alias = "endTime")]
    #[validate(length(max = 20))]
    pub end_time: String,
    #[serde(default)]
    #[validate(length(max = 10))]
    pub date: String,
}

#[derive(serde::Serialize, ToSchema)]
pub struct SportsStreamsResponse {
    pub success: bool,
    pub query: String,
    pub count: usize,
    pub streams: Vec<SportsStream>,
}

#[derive(serde::Serialize, ToSchema)]
pub struct SportsDataResponse {
    pub success: bool,
    pub data: serde_json::Value,
}

fn default_match_type() -> String {
    "MTFootball".to_string()
}

fn default_sport_type() -> String {
    "football".to_string()
}

fn default_league_id() -> String {
    "0".to_string()
}

fn default_latest_count() -> u32 {
    5
}

fn current_utc_day_bounds_ms() -> (String, String) {
    let now = Utc::now();
    let start = Utc
        .with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0)
        .single()
        .unwrap_or(now);
    let end = start + chrono::Duration::days(1) - chrono::Duration::milliseconds(1);
    (
        start.timestamp_millis().to_string(),
        end.timestamp_millis().to_string(),
    )
}

fn utc_day_bounds_ms_for_date(date: &str) -> Option<(String, String)> {
    let date = NaiveDate::parse_from_str(date.trim(), "%Y-%m-%d").ok()?;
    let start = Utc
        .with_ymd_and_hms(date.year(), date.month(), date.day(), 0, 0, 0)
        .single()?;
    let end = start + chrono::Duration::days(1) - chrono::Duration::milliseconds(1);
    Some((
        start.timestamp_millis().to_string(),
        end.timestamp_millis().to_string(),
    ))
}

fn apply_default_day_bounds(start_time: &mut String, end_time: &mut String, date: &str) {
    if !date.trim().is_empty() {
        if let Some((start, end)) = utc_day_bounds_ms_for_date(date) {
            *start_time = start;
            *end_time = end;
            return;
        }
    }

    if start_time.trim().is_empty() || end_time.trim().is_empty() {
        let (start, end) = current_utc_day_bounds_ms();
        if start_time.trim().is_empty() {
            *start_time = start;
        }
        if end_time.trim().is_empty() {
            *end_time = end;
        }
    }
}

fn match_count(data: &serde_json::Value) -> usize {
    data.get("list")
        .and_then(|list| list.as_array())
        .map(|groups| {
            groups
                .iter()
                .map(|group| {
                    group
                        .get("matchList")
                        .and_then(|matches| matches.as_array())
                        .map(Vec::len)
                        .unwrap_or(0)
                })
                .sum()
        })
        .unwrap_or(0)
}

fn wrap_sports_streams(
    streams: Vec<SportsStream>,
    proxy_base: &str,
    secret: &str,
) -> Vec<SportsStream> {
    streams
        .into_iter()
        .map(|mut stream| {
            if stream.url.contains(proxy_base)
                && (stream.url.contains("/m3u8-proxy") || stream.url.contains("/ts-proxy"))
            {
                return stream;
            }

            let headers = stream.headers.clone();
            let token = sign_proxy_url(&stream.url, secret, headers);
            let route = if stream.url.to_lowercase().contains(".m3u8") {
                "m3u8-proxy"
            } else {
                "ts-proxy"
            };

            stream.url = format!("{}/{}?{}", proxy_base, route, token.to_query());
            stream.headers = None;
            stream
        })
        .collect()
}

#[utoipa::path(
    get,
    path = "/api/aisports/leagues",
    responses(
        (status = 200, description = "Raw AISports league tabs", body = SportsDataResponse),
    ),
)]
pub async fn leagues_handler(State(state): State<AppState>) -> Response {
    match state.aisports.get_league_tabs().await {
        Ok(data) => (
            StatusCode::OK,
            serde_json::to_string(&SportsDataResponse {
                success: true,
                data,
            })
            .unwrap(),
        )
            .into_response(),
        Err(e) => {
            warn!("[api] AiSports leagues failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({"success": false, "error": e.to_string()}).to_string(),
            )
                .into_response()
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/aisports/league-tabs",
    responses(
        (status = 200, description = "Raw AISports league tabs", body = SportsDataResponse),
    ),
)]
pub async fn league_tabs_handler(State(state): State<AppState>) -> Response {
    match state.aisports.get_league_tabs().await {
        Ok(data) => (
            StatusCode::OK,
            serde_json::to_string(&SportsDataResponse {
                success: true,
                data,
            })
            .unwrap(),
        )
            .into_response(),
        Err(e) => {
            warn!("[api] AiSports league tabs failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({"success": false, "error": e.to_string()}).to_string(),
            )
                .into_response()
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/aisports/matches",
    params(
        ("leagueId" = Option<String>, Query, description = "League id, defaults to 0 for all"),
    ),
    responses(
        (status = 200, description = "Raw AISports match-list-v5 data", body = SportsDataResponse),
    ),
)]
pub async fn matches_handler(
    State(state): State<AppState>,
    Query(params): Query<LeagueMatchesQuery>,
) -> Response {
    if let Err(e) = params.validate() {
        return (
            StatusCode::BAD_REQUEST,
            json!({"success": false, "error": format!("Invalid query: {}", e)}).to_string(),
        )
            .into_response();
    }

    match state.aisports.get_match_list_v5(&params.league_id).await {
        Ok(data) => (
            StatusCode::OK,
            serde_json::to_string(&SportsDataResponse {
                success: true,
                data,
            })
            .unwrap(),
        )
            .into_response(),
        Err(e) => {
            warn!("[api] AiSports match list failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({"success": false, "error": e.to_string()}).to_string(),
            )
                .into_response()
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/aisports/matches/grouped",
    params(
        ("status" = Option<i32>, Query, description = "Match status filter, defaults to 0"),
        ("matchType" = Option<String>, Query, description = "Sport type, defaults to football"),
        ("date" = Option<String>, Query, description = "UTC date as YYYY-MM-DD; overrides startTime/endTime when present"),
        ("startTime" = Option<String>, Query, description = "Start timestamp in milliseconds; defaults to current UTC day start"),
        ("endTime" = Option<String>, Query, description = "End timestamp in milliseconds; defaults to current UTC day end"),
    ),
    responses(
        (status = 200, description = "Raw AISports grouped match-list-v3 data", body = SportsDataResponse),
    ),
)]
pub async fn grouped_matches_handler(
    State(state): State<AppState>,
    Query(mut params): Query<GroupedMatchesQuery>,
) -> Response {
    if let Err(e) = params.validate() {
        return (
            StatusCode::BAD_REQUEST,
            json!({"success": false, "error": format!("Invalid query: {}", e)}).to_string(),
        )
            .into_response();
    }

    apply_default_day_bounds(&mut params.start_time, &mut params.end_time, &params.date);

    match state
        .aisports
        .get_match_list_v3(
            params.status,
            &params.match_type,
            &params.start_time,
            &params.end_time,
        )
        .await
    {
        Ok(data) => (
            StatusCode::OK,
            serde_json::to_string(&SportsDataResponse {
                success: true,
                data,
            })
            .unwrap(),
        )
            .into_response(),
        Err(e) => {
            warn!("[api] AiSports grouped match list failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({"success": false, "error": e.to_string()}).to_string(),
            )
                .into_response()
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/aisports/matches/live",
    params(
        ("matchType" = Option<String>, Query, description = "Sport type, defaults to football"),
        ("date" = Option<String>, Query, description = "UTC date as YYYY-MM-DD; overrides startTime/endTime when present"),
        ("startTime" = Option<String>, Query, description = "Start timestamp in milliseconds; defaults to current UTC day start"),
        ("endTime" = Option<String>, Query, description = "End timestamp in milliseconds; defaults to current UTC day end"),
    ),
    responses(
        (status = 200, description = "Raw live AISports match-list-v3 data", body = SportsDataResponse),
    ),
)]
pub async fn live_matches_handler(
    State(state): State<AppState>,
    Query(mut params): Query<MatchFilterTabsQuery>,
) -> Response {
    if let Err(e) = params.validate() {
        return (
            StatusCode::BAD_REQUEST,
            json!({"success": false, "error": format!("Invalid query: {}", e)}).to_string(),
        )
            .into_response();
    }

    apply_default_day_bounds(&mut params.start_time, &mut params.end_time, &params.date);

    match state
        .aisports
        .get_match_list_v3(2, &params.match_type, &params.start_time, &params.end_time)
        .await
    {
        Ok(data) => (
            StatusCode::OK,
            serde_json::to_string(&SportsDataResponse {
                success: true,
                data,
            })
            .unwrap(),
        )
            .into_response(),
        Err(e) => {
            warn!("[api] AiSports live match list failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({"success": false, "error": e.to_string()}).to_string(),
            )
                .into_response()
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/aisports/matches/filter-tabs",
    params(
        ("matchType" = Option<String>, Query, description = "Sport type, defaults to football"),
        ("date" = Option<String>, Query, description = "UTC date as YYYY-MM-DD; overrides startTime/endTime when present"),
        ("startTime" = Option<String>, Query, description = "Start timestamp in milliseconds; defaults to current UTC day start"),
        ("endTime" = Option<String>, Query, description = "End timestamp in milliseconds; defaults to current UTC day end"),
    ),
    responses(
        (status = 200, description = "AISports match-list-v3 filter tab data", body = SportsDataResponse),
    ),
)]
pub async fn filter_tabs_handler(
    State(state): State<AppState>,
    Query(mut params): Query<MatchFilterTabsQuery>,
) -> Response {
    if let Err(e) = params.validate() {
        return (
            StatusCode::BAD_REQUEST,
            json!({"success": false, "error": format!("Invalid query: {}", e)}).to_string(),
        )
            .into_response();
    }

    apply_default_day_bounds(&mut params.start_time, &mut params.end_time, &params.date);

    let specs = [
        (0, "all", "All"),
        (1, "upcoming", "Upcoming"),
        (2, "live", "Live"),
        (3, "ended", "Ended"),
    ];
    let mut tabs = Vec::with_capacity(specs.len());

    for (status, key, label) in specs {
        match state
            .aisports
            .get_match_list_v3(
                status,
                &params.match_type,
                &params.start_time,
                &params.end_time,
            )
            .await
        {
            Ok(data) => {
                tabs.push(json!({
                    "status": status,
                    "key": key,
                    "label": label,
                    "count": match_count(&data),
                    "data": data
                }));
            }
            Err(e) => {
                warn!("[api] AiSports filter tab {} failed: {}", key, e);
                tabs.push(json!({
                    "status": status,
                    "key": key,
                    "label": label,
                    "count": 0,
                    "error": e.to_string(),
                    "data": {"list": []}
                }));
            }
        }
    }

    (
        StatusCode::OK,
        serde_json::to_string(&SportsDataResponse {
            success: true,
            data: json!({
                "matchType": params.match_type,
                "startTime": params.start_time,
                "endTime": params.end_time,
                "tabs": tabs
            }),
        })
        .unwrap(),
    )
        .into_response()
}

#[utoipa::path(
    get,
    path = "/api/aisports/home",
    params(
        ("leagueId" = Option<String>, Query, description = "League id, defaults to 0 for all"),
    ),
    responses(
        (status = 200, description = "Raw AISports aggregate home data", body = SportsDataResponse),
    ),
)]
pub async fn home_handler(
    State(state): State<AppState>,
    Query(params): Query<LeagueMatchesQuery>,
) -> Response {
    if let Err(e) = params.validate() {
        return (
            StatusCode::BAD_REQUEST,
            json!({"success": false, "error": format!("Invalid query: {}", e)}).to_string(),
        )
            .into_response();
    }

    match state.aisports.get_aggregate(&params.league_id).await {
        Ok(data) => (
            StatusCode::OK,
            serde_json::to_string(&SportsDataResponse {
                success: true,
                data,
            })
            .unwrap(),
        )
            .into_response(),
        Err(e) => {
            warn!("[api] AiSports aggregate failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({"success": false, "error": e.to_string()}).to_string(),
            )
                .into_response()
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/aisports/latest",
    params(
        ("matchType" = Option<String>, Query, description = "AiSports match type, defaults to MTFootball"),
        ("count" = Option<u32>, Query, description = "Number of matches to return, 1-50"),
    ),
    responses(
        (status = 200, description = "Raw latest AiSports matches", body = SportsDataResponse),
    ),
)]
pub async fn latest_handler(
    State(state): State<AppState>,
    Query(params): Query<LatestMatchesQuery>,
) -> Response {
    if let Err(e) = params.validate() {
        return (
            StatusCode::BAD_REQUEST,
            json!({"success": false, "error": format!("Invalid query: {}", e)}).to_string(),
        )
            .into_response();
    }

    match state
        .aisports
        .get_latest_matches(&params.match_type, params.count)
        .await
    {
        Ok(data) => (
            StatusCode::OK,
            serde_json::to_string(&SportsDataResponse {
                success: true,
                data,
            })
            .unwrap(),
        )
            .into_response(),
        Err(e) => {
            warn!("[api] AiSports latest failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({"success": false, "error": e.to_string()}).to_string(),
            )
                .into_response()
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/sports/streams",
    params(
        ("q" = String, Query, description = "Match name or team name to search for"),
    ),
    responses(
        (status = 200, description = "Sports stream results", body = SportsStreamsResponse),
        (status = 404, description = "No streams found"),
    ),
)]
pub async fn streams_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<SportsStreamQuery>,
) -> Response {
    if let Err(e) = params.validate() {
        return (
            StatusCode::BAD_REQUEST,
            json!({"success": false, "error": format!("Invalid query: {}", e)}).to_string(),
        )
            .into_response();
    }

    let query = params.q.trim();
    if query.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            json!({"success": false, "error": "Query parameter 'q' is required"}).to_string(),
        )
            .into_response();
    }

    info!("[api] Sports Streams: search='{}'", query);

    let proxy_base = dynamic_proxy_base(&headers, &state.proxy_base_url);

    match state.aisports.search_streams(query).await {
        Ok(streams) if !streams.is_empty() => {
            let streams = if state.config.enable_proxy {
                wrap_sports_streams(streams, &proxy_base, &state.secret)
            } else {
                streams
            };

            (
                StatusCode::OK,
                serde_json::to_string(&SportsStreamsResponse {
                    success: true,
                    query: query.to_string(),
                    count: streams.len(),
                    streams,
                })
                .unwrap(),
            )
                .into_response()
        }
        Ok(_) => {
            info!("[api] No sports streams found for '{}'", query);
            (
                StatusCode::OK,
                serde_json::to_string(&SportsStreamsResponse {
                    success: true,
                    query: query.to_string(),
                    count: 0,
                    streams: vec![],
                })
                .unwrap(),
            )
                .into_response()
        }
        Err(e) => {
            warn!("[api] AiSports search failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({"success": false, "error": e.to_string()}).to_string(),
            )
                .into_response()
        }
    }
}
