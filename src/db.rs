use std::env;

use chrono::{DateTime, Utc};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::prelude::FromRow;

pub use sqlx::{query, query_as, Error, MySqlPool};

static DB_POOL: OnceCell<MySqlPool> = OnceCell::new();

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ConfigHash {
  pub version: String,
  pub hash: String,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct GameServer {
  pub id: i64,
  pub game: String,
  pub short_name: String,
  pub long_name: String,
  pub version: String,
  pub pre_version: Option<String>,
  pub server: i32,
  pub patch_time: DateTime<Utc>,
  pub configuration: Value,
  #[sqlx(json)]
  pub hotfix_hash: Vec<ConfigHash>,
  pub created_at: DateTime<Utc>,
  pub updated_at: DateTime<Utc>,
}

fn version_to_int(v: &str) -> u64 {
  v.chars()
    .filter(|c| c.is_ascii_digit())
    .collect::<String>()
    .parse::<u64>()
    .unwrap_or(0)
}

impl GameServer {
  pub fn get_config<T>(&self) -> T
  where
    T: for<'de> Deserialize<'de> + Default,
  {
    if self.configuration.is_null()
      || (self.configuration.is_object() && self.configuration.as_object().unwrap().is_empty())
    {
      return T::default();
    }

    serde_json::from_value(self.configuration.clone()).unwrap_or_default()
  }

  // ==

  fn sorted(&self) -> Vec<&ConfigHash> {
    let mut configs: Vec<&ConfigHash> = self.hotfix_hash
      .iter()
      .collect();

    configs.sort_by(|a, b| version_to_int(&b.version).cmp(&version_to_int(&a.version)));
    configs
  }

  pub fn get_hash_by_version(&self, version: &str) -> Option<&ConfigHash> {
    self.hotfix_hash
      .iter()
      .find(|cfg| cfg.version == version)
  }

  pub fn get_hash_by_index(&self, index: usize) -> Option<&ConfigHash> {
    let configs  = self.sorted();
    configs.get(index).copied()
  }

  pub fn get_latest_hash(&self) -> Option<&ConfigHash> {
    self.get_hash_by_index(0)
  }

  pub fn get_all_hash(&self) -> Vec<&ConfigHash> {
    self.sorted()
  }
}

// ===

pub async fn init_db_pool() -> sqlx::Result<()> {
  let db_uri = env::var("DATABASE_URI").expect("DATABASE_URI must be set");
  tracing::debug!("Connecting to {}", db_uri);

  let pool = sqlx::mysql::MySqlPoolOptions::new()
    .max_connections(5)
    .connect(&db_uri)
    .await?;

  DB_POOL.set(pool).map_err(|_| {
    sqlx::Error::Configuration(Box::new(std::io::Error::new(
      std::io::ErrorKind::AlreadyExists,
      "DB_POOL already initialized!",
    )))
  })?;

  Ok(())
}

pub fn get_db_pool() -> MySqlPool {
  DB_POOL
    .get()
    .expect("Database pool not initialized. Call init_db_pool() first!")
    .clone()
}

pub async fn get_game_servers() -> Result<Vec<GameServer>, sqlx::Error> {
  let pool = get_db_pool();

  query_as::<_, GameServer>("SELECT * from `game_servers`")
    .fetch_all(&pool)
    .await
}

pub async fn get_game_server_by_game_name(game_name: &str) -> Result<GameServer, sqlx::Error> {
  let pool = get_db_pool();

  query_as::<_, GameServer>("SELECT * from `game_servers` WHERE `game` = ?")
    .bind(game_name)
    .fetch_one(&pool)
    .await
}

pub async fn get_game_servers_by_game_name(game_name: &str) -> Result<Vec<GameServer>, sqlx::Error> {
  let pool = get_db_pool();

  query_as::<_, GameServer>("SELECT * from `game_servers` WHERE `game` = ?")
    .bind(game_name)
    .fetch_all(&pool)
    .await
}

pub async fn update_server_status(short_name: &str, status: i32) {
  let pool = get_db_pool();
  let now: DateTime<Utc> = Utc::now();

  let res = query("UPDATE `game_servers` SET `server` = ?, `updated_at` = ? WHERE `short_name` = ?")
    .bind(status)
    .bind(now)
    .bind(short_name)
    .execute(&pool)
    .await;

  tracing::debug!("{:?}", res);
}

pub async fn update_game_version(short_name: &str, version: &str) {
  let pool = get_db_pool();
  let now: DateTime<Utc> = Utc::now();

  let res = query("UPDATE `game_servers` SET `version` = ?, `patch_time` = ?, `updated_at` = ? WHERE `short_name` = ?")
    .bind(version)
    .bind(now)
    .bind(now)
    .bind(short_name)
    .execute(&pool)
    .await;

  tracing::debug!("{:?}", res);
}

pub async fn update_game_configuration(short_name: &str, configuration: &str) {
  let pool = get_db_pool();
  let now: DateTime<Utc> = Utc::now();

  let res = query("UPDATE `game_servers` SET `configuration` = ?, `patch_time` = ?, `updated_at` = ? WHERE `short_name` = ?")
    .bind(configuration)
    .bind(now)
    .bind(now)
    .bind(short_name)
    .execute(&pool)
    .await;

  tracing::debug!("{:?}", res);
}