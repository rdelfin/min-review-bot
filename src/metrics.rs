use anyhow::Result;
use datadog_statsd::Client as DdClient;
use lazy_static::lazy_static;
use opentelemetry::{
    metrics::{Histogram, Meter, MeterProvider},
    sdk::{
        metrics::reader::{DefaultAggregationSelector, DefaultTemporalitySelector},
        Resource,
    },
};
use opentelemetry_api::KeyValue;
use std::{path::Path, sync::RwLock, time::Duration};

lazy_static! {
    static ref DD_CLIENT: RwLock<Option<MetricsReporter>> = RwLock::new(None);
}

pub struct MetricsReporter {
    dd_client: DdClient,
    loop_duration_timer: Histogram<f64>,
    loop_load_hist: Histogram<f64>,
}

impl MetricsReporter {
    fn new<P: AsRef<Path>>(dd_socket_path: P) -> Result<MetricsReporter> {
        let dd_client = DdClient::with_uds_socket(
            dd_socket_path.as_ref(),
            "min_review_bot",
            Some(vec!["source:min-review-bot", "service:min-review-bot"]),
        )?;
        // Initialize the MeterProvider with the stdout Exporter.
        let meter = Self::init_meter()?;

        // Create a meter from the above MeterProvider.
        let loop_duration_timer = meter.f64_histogram("loop_duration_ms").init();
        let loop_load_hist = meter.f64_histogram("loop_load").init();
        Ok(MetricsReporter {
            dd_client,
            loop_duration_timer,
            loop_load_hist,
        })
    }

    pub fn initialise<P: AsRef<Path>>(dd_socket_path: P) -> Result<()> {
        *DD_CLIENT.write().unwrap() = Some(Self::new(dd_socket_path)?);
        Ok(())
    }

    pub fn report_loop_data(true_duration: Duration, max_duration: Duration) {
        let loop_load = true_duration.as_secs_f64() / max_duration.as_secs_f64();
        let loop_duration_ms = true_duration.as_secs_f64() * 1000.;

        let lg = DD_CLIENT.read().unwrap();
        let mr = match lg.as_ref() {
            Some(c) => c,
            None => {
                return;
            }
        };
        mr.dd_client
            .timer("loop_duration", true_duration.as_secs_f64() * 1000., &None);
        mr.dd_client.gauge("loop_load", loop_load, &None);
        mr.loop_duration_timer.record(loop_duration_ms, &[]);
        mr.loop_load_hist.record(loop_load, &[]);
    }

    fn init_meter() -> Result<Meter> {
        Ok(opentelemetry_otlp::new_pipeline()
            .metrics(opentelemetry::sdk::runtime::Tokio)
            .with_exporter(opentelemetry_otlp::new_exporter().tonic())
            .with_resource(Resource::new(vec![KeyValue::new(
                "service.name",
                "min_review_bot",
            )]))
            .with_period(Duration::from_secs(3))
            .with_timeout(Duration::from_secs(10))
            .with_aggregation_selector(DefaultAggregationSelector::new())
            .with_temporality_selector(DefaultTemporalitySelector::new())
            .build()?
            .meter("min_review_bot"))
    }
}
