use std::{env, time::Duration};

use reqwest::{Client, Proxy};

pub async fn fetch_data(url: &str) -> Option<String> {
  let proxy = env::var("PROXIES").expect("PROXIES must be set");
  let proxy = Proxy::all(proxy).expect("Failed to create proxy");

  let client = Client::builder()
    .proxy(proxy)
    .timeout(Duration::from_secs(30))
    .build()
    .expect("Unable to create client builder!");

  let response = client
    .get(url)
    .send().await
    .expect("Unable to get response data!");

  let status = response.status();
  if !status.is_success() {
    tracing::error!("An error occured! {:?}", status);
    return None;
  }

  let bytes = match response.bytes().await {
    Ok(b) => b,
    Err(e) => {
      tracing::error!("Failed to read bytes: {}", e);
      return None;
    }
  };

  let data = String::from_utf8_lossy(&bytes).to_string();
  Some(data.trim_start_matches('\u{feff}').to_string())
}