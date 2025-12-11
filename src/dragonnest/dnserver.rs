use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use std::time::Duration;

use once_cell::sync::Lazy;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::time::timeout;

use crate::db::{get_game_servers_by_game_name, update_server_status};
use crate::schemas::DnGameConfig;

#[derive(Default)]
pub struct Cache {
  status: HashMap<String, i32>,
  attempt: HashMap<String, u32>,
}

impl Cache {
  fn init_server(&mut self, name: &str) {
    self.status.entry(name.to_string()).or_insert(0);
    self.attempt.entry(name.to_string()).or_insert(0);
  }
}

static GLOBAL_CACHE: Lazy<Arc<Mutex<Cache>>> = Lazy::new(|| Arc::new(Mutex::new(Cache::default())));
const MAX_RETRY: u32 = 5;

fn to_hex_string(bytes: &[u8]) -> String {
  bytes
    .iter()
    .map(|b| format!("0x{:02X}", b))
    .collect::<Vec<_>>()
    .join(" ")
}

async fn check_server(name: &str, ip: &str, port: &str) -> tokio::io::Result<bool> {
  let proxy = env::var("PROXIES").expect("PROXIES must be set");
  let target = format!("{}:{}", ip, port);
  
  tracing::info!("[{}] Connecting to {} using proxy {}", name, target, proxy);

  let mut stream = TcpStream::connect(proxy).await?;
  stream.set_nodelay(true)?;
  // stream.set_keepalive(Some(Duration::from_secs(15)))?;
  
  // basic auth: Authorization: Basic <base64(user:pass)>
  let connect_request = format!(
    "CONNECT {} HTTP/1.1\r\nHost: {}\r\nProxy-Connection: Keep-Alive\r\n\r\n",
    target, target
  );

  stream.write_all(connect_request.as_bytes()).await?;
  stream.flush().await?;
  
  // read response
  let mut response_buffer = Vec::new();
  let mut buf = [0u8; 64];

  // wait 3 secs for response
  let read_result = timeout(Duration::from_secs(3), async {
    loop {
      let n = stream.read(&mut buf).await?;
      if n == 0 {
        break; // connection closed
      }

      response_buffer.extend_from_slice(&buf[..n]);
      // stop if header is completed
      if response_buffer.windows(4).any(|w| w == b"\r\n\r\n") {
        break;
      }
    }

    Ok::<_, std::io::Error>(())
  }).await;

  if read_result.is_err() {
    tracing::error!("[{}] Connection timeout!", name);
    return Ok(false);
  }

  let header = String::from_utf8_lossy(&response_buffer);
  if let Some(first_line) = header.lines().next() {
    if first_line.contains("200") {
      tracing::debug!("[{}] Proxy connection success!", name);

      // read server response
      if let Ok(Ok(n)) = timeout(Duration::from_millis(5000), stream.read(&mut buf)).await {
        if n > 0 {
          tracing::info!("[{}] Response from server ({} bytes): {}", name, n, to_hex_string(&buf));
          return Ok(true);
        } else {
          tracing::error!("[{}] No data from target, wrong server?", name);
          return Ok(false);
        }
      } else {
        tracing::error!("[{}] Connection timeout!", name);
        return Ok(false);
      }

    } else {
      tracing::error!("[{}] Proxy connection failed", name);
      return Ok(false);
    }
  }

  Ok(false)
}

async fn handle_server_result(short_name: &str, long_name: &str, status: bool) {
  let mut cache_lock = GLOBAL_CACHE.lock().await;
  cache_lock.init_server(short_name);

  let prev_status = *cache_lock.status.get(short_name).unwrap_or(&-1);
  let prev_attempt = *cache_lock.attempt.get(short_name).unwrap_or(&0);

  let curr_status = if status { 1 } else { 0 };

  if curr_status != prev_status {
    let new_attempt = prev_attempt + 1;
    cache_lock.attempt.insert(short_name.to_string(), new_attempt);

    tracing::debug!("[{}] Check attempt: {}", long_name, new_attempt);

    if new_attempt >= MAX_RETRY || curr_status == 1 {
      tracing::debug!("[{}] > Sending notification ...", long_name);

      // Update database
      update_server_status(short_name, curr_status).await;

      // TODO: Send webhook

      cache_lock.status.insert(short_name.to_string(), curr_status);
      cache_lock.attempt.insert(short_name.to_string(), 0);

      tracing::debug!("[{}] Status changed to {} (attempt reset)", long_name, curr_status);
    }
  } else {
    cache_lock.attempt.insert(short_name.to_string(), 0);
  }

  tracing::info!("[{}] Closed!", long_name);
}

pub async fn handle_cron() {
  let _game_servers = get_game_servers_by_game_name("dn").await;
  if let Ok(game_servers) = _game_servers {
    for server in game_servers {
      let game_config: DnGameConfig = server.get_config();
      let ips = &game_config.ip;

      for (i, p) in ips.iter().enumerate() {
        let server_name = format!("DN-{} {}", server.short_name.to_uppercase(), i + 1);
        let server_ip: Vec<&str> = p.split(":").collect();

        let ret = check_server(&server_name, server_ip[0], server_ip[1]).await.unwrap_or(false);
        handle_server_result(&server.short_name, &server_name, ret).await;
        tokio::time::sleep(Duration::from_millis(500)).await;
      }
    }
  }
}
