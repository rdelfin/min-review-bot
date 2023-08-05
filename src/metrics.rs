use anyhow::Result;
use datadog_statsd::Client as DdClient;
use lazy_static::lazy_static;
use std::{path::Path, sync::RwLock, time::Duration};

lazy_static! {
    static ref DD_CLIENT: RwLock<Option<MetricsReporter>> = RwLock::new(None);
}

pub struct MetricsReporter {
    dd_client: DdClient,
}

impl MetricsReporter {
    fn new<P: AsRef<Path>>(dd_socket_path: P) -> Result<MetricsReporter> {
        let dd_client = DdClient::with_uds_socket(
            dd_socket_path.as_ref(),
            "min_review_bot",
            Some(vec!["source:min-review-bot", "service:min-review-bot"]),
        )?;
        Ok(MetricsReporter { dd_client })
    }

    pub fn initialise<P: AsRef<Path>>(dd_socket_path: P) -> Result<()> {
        *DD_CLIENT.write().unwrap() = Some(Self::new(dd_socket_path)?);
        Ok(())
    }

    pub fn report_loop_data(true_duration: Duration, max_duration: Duration) {
        let lg = DD_CLIENT.read().unwrap();
        let client = &match lg.as_ref() {
            Some(client) => client,
            None => return,
        }
        .dd_client;
        let load = true_duration.as_secs_f64() / max_duration.as_secs_f64();
        client.timer("loop_duration", true_duration.as_secs_f64() * 1000., &None);
        client.gauge("loop_load", load, &None);
    }
}
