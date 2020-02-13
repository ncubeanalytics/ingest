//! Kafka producer wrapper.

use std::ops::Deref;
use std::sync::Arc;

use bytes::Bytes;
use futures::FutureExt;
use rdkafka::{
    error::KafkaError,
    producer::{FutureProducer, FutureRecord},
    util::Timeout,
    ClientConfig,
};
use tracing::{error, trace};

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

    pub async fn send(&self, data: Bytes) -> Result<()> {
        let record = FutureRecord::to(&self.config.topic)
            .key("")
            .payload(data.as_ref());

        self.producer
            .send(record, -1)
            .map(|delivery_status| match delivery_status {
                Err(_) => {
                    // means that something is wrong with the producer
                    error!("Kafka producer internal channel canceled");
                    Err(KafkaError::Canceled.into())
                }

                Ok(Err((e, _))) => Err(e.into()),

                _ => {
                    trace!("Message successfully sent to kafka broker");
                    Ok(())
                }
            })
            .await
    }

    pub fn stop(self) {
        trace!("Flushing kafka producer");

        self.producer.flush(Timeout::Never);

        trace!("Done flushing kafka producer");
    }
}
