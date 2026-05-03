use reqwest::Client;
use serde_json::Value;

fn api_base() -> String {
    std::env::var("TEST_API_URL").unwrap_or_else(|_| "http://127.0.0.1:24609".to_string())
}

#[tokio::test]
async fn test_health_endpoint() {
    let client = Client::new();
    let url = format!("{}/health", api_base());
    let resp = client.get(&url).send().await;

    match resp {
        Ok(r) => assert!(r.status().is_success(), "Health endpoint should return 200"),
        Err(_) => eprintln!("Skipping health test — API not running"),
    }
}

#[tokio::test]
async fn test_providers_endpoint() {
    let client = Client::new();
    let url = format!("{}/api/providers", api_base());
    let resp = client.get(&url).send().await;

    if let Ok(r) = resp {
        assert!(r.status().is_success());
        let body: Value = r.json().await.unwrap();
        assert!(body.is_array(), "Providers should return an array");
    }
}

#[tokio::test]
async fn test_hitv_browse_endpoint() {
    let client = Client::new();
    let url = format!("{}/api/hitv/browse?sort=Hottest", api_base());
    let resp = client.get(&url).send().await;

    if let Ok(r) = resp {
        assert!(r.status().is_success());
        let body: Value = r.json().await.unwrap();
        assert!(body.is_array(), "Browse should return an array");
    }
}
