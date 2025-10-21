use std::{collections::HashSet, env, time::Duration};

use regex::Regex;
use reqwest::{Client, Proxy};

use crate::{db::{get_game_server_by_game_name, update_game_configuration, update_game_version}, schemas::DnGameConfig};

async fn get_data(url: &str) -> Option<String> {
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

pub async fn handle_cron() {
  let _game_servers = get_game_server_by_game_name("dn").await;
  if let Ok(game_servers) = _game_servers {
    for server in game_servers {
      let mut game_config: DnGameConfig = serde_json::from_value(server.configuration).expect("Unable to parse configuration field!");
      tracing::info!("Checking '{}' game config", server.long_name);

      let config_url = &game_config.patch_config_list;
      tracing::debug!("* Patch config url: {}", config_url);

      let _patch_config_data = get_data(config_url).await;
      let Some(patch_config_data) = _patch_config_data else {
        continue;
      };

      // find server ips
      let re = Regex::new(r#"(?i)<login\s+addr="([\d.]+)".*?port="(\d+)"\s*/?>"#).unwrap();
      let mut server_ips: Vec<String> = re
        .captures_iter(&patch_config_data)
        .map(|cap| format!("{}:{}", &cap[1], &cap[2]))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
      
      // sort first
      game_config.ip.sort();
      server_ips.sort();

      tracing::info!("* Server IPs: {:?}", server_ips);
    
      // updating server IPs
      if game_config.ip != server_ips {
        game_config.ip = server_ips;
        
        update_game_configuration(
          &server.short_name,
          &serde_json::to_string(&game_config).unwrap()
        ).await;
      }
        
      // find latest version
      let re = Regex::new(r#"(?i)<version\s+addr="([^"]+)"\s*/?>"#).unwrap();
      let cap = re.captures(&patch_config_data).unwrap();

      let patch_version_url = format!("{}PatchInfoServer.cfg", &cap[1].replace("http:", "https:"));
      tracing::debug!("* Version config url: {}", patch_version_url);

      let _version_data = get_data(&patch_version_url).await;
      let Some(version_data) = _version_data else {
        continue;
      };

      // parse version number
      let re = Regex::new(r#"(?i)version\s+(\d+)"#).unwrap();
      let cap = re.captures(&version_data).unwrap();
      let version_number: i32 = cap[1].parse().unwrap_or(0);

      tracing::info!("* Latest version: {}", version_number);

      // start updating data
      if version_number > server.version.parse().unwrap_or(0) {
        tracing::info!("* New version detected!");
        
        // TODO: Send webhook
        
        // update version
        update_game_version(&server.short_name, &version_number.to_string()).await;
      } else {
        tracing::info!("* Version is up-to-date!");
      }
      
      tracing::info!("Done!");
    }
  }
}