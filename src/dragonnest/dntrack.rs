use std::{env, time::Duration};

use quick_xml::de::from_str;
use regex::Regex;
use reqwest::{Client, Proxy};
use serde::Deserialize;

use crate::{db::{get_game_server_by_game_name, update_game_configuration, update_game_version}, schemas::DnGameConfig};

#[derive(Debug, Deserialize)]
struct Document {
  #[serde(rename = "ChannelingList", alias = "ChannelList")]
  channel_list: Vec<ChannelingList>,
}

#[derive(Debug, Deserialize)]
struct ChannelingList {
  // #[serde(rename = "@AreaShowName")]
  // area_show_name: Option<String>,
  // #[serde(rename = "@AreaType")]
  // area_type: Option<u32>,
  // #[serde(rename = "@channelingName", alias = "@channel_name")]
  // channeling_name: String,
  #[serde(rename = "Local")]
  locals: Vec<Local>,
}

#[derive(Debug, Deserialize)]
struct Local {
  // #[serde(rename = "@localName", alias = "@local_name")]
  // local_name: String,
  // #[serde(rename = "@new")]
  // new_flag: Option<u8>,
  // #[serde(rename = "@open")]
  // open: Option<u8>,
  // #[serde(rename = "@partitionId")]
  // partition_id: Option<u32>,
  #[serde(rename = "Version", alias = "version")]
  version: Version,
  // #[serde(rename = "Update", alias = "update")]
  // update: Update,
  #[serde(rename = "Login", alias = "login")]
  login: Vec<Login>,
}

#[derive(Debug, Deserialize)]
struct Version {
  #[serde(rename = "@addr")]
  addr: String,
}

// #[derive(Debug, Deserialize)]
// struct Update {
//   #[serde(rename = "@addr")]
//   addr: String,
// }

#[derive(Debug, Deserialize)]
struct Login {
  #[serde(rename = "@addr")]
  addr: String,
  #[serde(rename = "@port")]
  port: String,
}

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

      let xml_data: Document = from_str(&patch_config_data).expect("Unable to parse xml data!");
      let selected_local = &xml_data.channel_list[0].locals[0];

      // find server ips
      let mut server_ips: Vec<String> = selected_local.login
        .iter()
        .map(|f| {
          format!("{}:{}", f.addr, f.port)
        })
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
      let patch_version_url = format!("{}PatchInfoServer.cfg", selected_local.version.addr.replace("http:", "https:"));
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