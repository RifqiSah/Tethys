use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct DnGameConfig {
  pub ip: Vec<String>,
  #[serde(rename = "patchConfigList")]
  pub patch_config_list: String,
}