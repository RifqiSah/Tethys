#[macro_export]
macro_rules! add_cron_fn {
  ($scheduler:expr, $schedule:expr, $handler:path) => {
    $scheduler
      .add(
        ::tokio_cron_scheduler::Job::new_async($schedule, |_uuid, _l| {
          ::std::boxed::Box::pin(async move {
            ::tracing::debug!("Executing {}", stringify!($handler));
            $handler().await;
          })
        })
        .unwrap(),
      )
      .await
      .unwrap();
  };
}

#[macro_export]
macro_rules! add_cron_with_param_fn {
  ($scheduler:expr, $schedule:expr, $handler:path, $a:expr) => {
    let _a = $a.clone();
    $scheduler
      .add(
        ::tokio_cron_scheduler::Job::new_async($schedule, move |_uuid, _l| {
          let __a = _a.clone();
          ::std::boxed::Box::pin(async move {
            ::tracing::debug!("Executing {}", stringify!($handler));
            $handler(__a).await;
          })
        })
        .unwrap(),
      )
      .await
      .unwrap();
  };
}