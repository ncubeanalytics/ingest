//! Kafka producer wrapper.

use std::ops::Deref;
use std::sync::Arc;

use bytes::Bytes;
use futures::TryFutureExt;
use rdkafka::{
    producer::{FutureProducer, FutureRecord, Producer},
    util::Timeout,
    ClientConfig,
};
use tracing::trace;

use crate::config::Config;
use crate::error::Result;

/// Kafka producer wrapper.
/// Can be cheaply cloned.
#[derive(Clone)]
pub struct Kafka(Arc<KafkaInner>);

pub struct KafkaInner {
    producer: FutureProducer,
    config: Config,
}

impl Deref for Kafka {
    type Target = KafkaInner;

    fn deref(&self) -> &KafkaInner {
        &self.0
    }
}

impl Kafka {
    pub fn start(config: &Config) -> Result<Kafka> {
        let mut producer_config = ClientConfig::new();

        for (key, value) in &config.librdkafka_config {
            producer_config.set(key, value);
        }

        let producer = producer_config.create()?;

        Ok(Self(Arc::new(KafkaInner {
            producer,
            config: config.clone(),
        })))
    }

    pub async fn send(&self, data: Bytes, _tenant_id: i64) -> Result<()> {
        // let topic = topic_name(&self.config.topic_prefix, tenant_id);

        let record = FutureRecord::to(&self.config.topic)
            .key("")
            .payload(data.as_ref());

        self.producer
            .send(record, Timeout::Never)
            .map_err(|(error, _)| error.into())
            .map_ok(|_| {
                trace!(%self.config.topic, "Message successfully sent to kafka broker");
                ()
            })
            .await
    }

    pub fn stop(self) {
        trace!("Flushing kafka producer");

        self.producer.flush(Timeout::Never);

        trace!("Done flushing kafka producer");
    }
}

fn _topic_name(prefix: &str, tenant_id: i64) -> String {
    format!("{}{}", prefix, tenant_id)
}
