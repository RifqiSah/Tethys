#[macro_use]
mod macros;

pub mod db;
pub mod fetch;
pub mod schemas;
pub mod dragonnest;

use dotenvy::dotenv;
use crate::db::init_db_pool;

use tokio_cron_scheduler::JobScheduler;

async fn wait_for_shutdown_signal() {
  #[cfg(unix)]
  {
    use tokio::signal::unix::{signal, SignalKind};

    let mut sigterm = signal(SignalKind::terminate()).unwrap();
    let mut sigint = signal(SignalKind::interrupt()).unwrap();

    tokio::select! {
      _ = sigterm.recv() => println!(""),
      _ = sigint.recv() => println!(""),
    }
  }

  #[cfg(not(unix))]
  {
    tokio::signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
  }
}

async fn start_cron_job(shutdown_rx: tokio::sync::broadcast::Sender<()>) {
  let mut shutdown_rx = shutdown_rx.subscribe();
  let mut scheduler = JobScheduler::new().await.unwrap();

  add_cron_fn!(scheduler, "0 * * * * *", dragonnest::dntrack::handle_cron);
  add_cron_fn!(scheduler, "30 * * * * *", dragonnest::dnserver::handle_cron);

  scheduler.start().await.unwrap();

  let _ = shutdown_rx.recv().await;
  tracing::warn!("Received shutdown signal, exiting...");

  scheduler.shutdown().await.unwrap();
}

#[tokio::main]
async fn main() {
  dotenv().ok();
  
  // global logger
  tracing_subscriber::fmt()
    .with_env_filter(tracing_subscriber::EnvFilter::new("tethys=debug"))
    .with_file(false)
    .init();

  // init db pool
  let _ = init_db_pool().await;
  
  let (shutdown_tx, _) = tokio::sync::broadcast::channel::<()>(1);
  let mut shutdown_rx = shutdown_tx.subscribe();

  // Cron Module
  let shutdown_cron = shutdown_tx.clone();
  let cron_task = tokio::spawn(async move {
    start_cron_job(shutdown_cron).await;
  });

  // Shutdown channel
  tokio::select! {
    _ = wait_for_shutdown_signal() => {},
    _ = shutdown_rx.recv() => {}
  }

  let _ = shutdown_tx.send(());
  let _ = tokio::join!(cron_task);
}
