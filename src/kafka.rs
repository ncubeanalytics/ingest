//! Kafka producer wrapper.

use std::ops::Deref;
use std::sync::Arc;

use bytes::Bytes;
use futures::TryFutureExt;
use rdkafka::{
    producer::{FutureProducer, FutureRecord},
    util::Timeout,
    ClientConfig,
};
use tracing::trace;

use crate::config::Kafka as KafkaConfig;
use crate::error::Result;

/// Kafka producer wrapper.
/// Can be cheaply cloned.
#[derive(Clone)]
pub struct Kafka(Arc<KafkaInner>);

pub struct KafkaInner {
    producer: FutureProducer,
    config: KafkaConfig,
}

impl Deref for Kafka {
    type Target = KafkaInner;

    fn deref(&self) -> &KafkaInner {
        &self.0
    }
}

impl Kafka {
    pub fn start(config: KafkaConfig) -> Result<Kafka> {
        let producer = ClientConfig::new()
            .set("bootstrap.servers", &config.servers)
            .set("delivery.timeout.ms", &config.timeout_ms)
            .set("acks", &config.acks)
            .create()?;

        Ok(Self(Arc::new(KafkaInner { producer, config })))
    }

    pub async fn send(&self, data: Bytes, tenant_id: i64) -> Result<()> {
        let topic = topic_name(&self.config.topic_prefix, tenant_id);

        let record = FutureRecord::to(&topic).key("").payload(data.as_ref());

        self.producer
            .send(record, Timeout::Never)
            .map_err(|(error, _)| error.into())
            .map_ok(|_| {
                trace!(%topic, "Message successfully sent to kafka broker");
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

fn topic_name(prefix: &str, tenant_id: i64) -> String {
    format!("{}{}", prefix, tenant_id)
}
