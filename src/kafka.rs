//! Kafka producer wrapper.

use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Arc;

use bytes::Bytes;
use futures::future::join_all;
use futures::TryFutureExt;
use rdkafka::message::OwnedHeaders;
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

    pub async fn send(
        &self,
        data: Bytes,
        headers: HashMap<String, String>,
        _tenant_id: i64,
    ) -> Result<()> {
        // let topic = topic_name(&self.config.topic_prefix, tenant_id);

        let mut kafka_headers = OwnedHeaders::new_with_capacity(headers.len());
        for (key, val) in headers {
            kafka_headers = kafka_headers.add(&key, &val);
        }
        let record = FutureRecord::to(&self.config.topic)
            .key("")
            .payload(data.as_ref())
            .headers(kafka_headers);

        self.producer
            .send(record, Timeout::Never)
            .map_err(|(error, _)| error.into())
            .map_ok(|_| {
                trace!(%self.config.topic, "Message successfully sent to kafka broker");
                ()
            })
            .await
    }

    pub async fn send_many(
        &self,
        data: Vec<Bytes>,
        headers: HashMap<String, String>,
        _tenant_id: i64,
    ) -> Result<()> {
        let mut futures = Vec::with_capacity(data.len());

        // sending events in order can be configured,
        // although it appears a bit problematic (see message timeouts)
        // https://github.com/edenhill/librdkafka/blob/master/INTRODUCTION.md#reordering
        // https://stackoverflow.com/a/53379022/2440380

        // XXX: this does not guarantee sending in order due to internal retries of
        // FutureProducer.send() in cases where the local queue is full. need to fix this so that
        // such retries happen while taking order into account
        // https://github.com/fede1024/rust-rdkafka/issues/468
        for d in data {
            futures.push(self.send(d, headers.clone(), _tenant_id))
        }

        join_all(futures) // XXX: does join_all execute in order?
            .await
            .into_iter()
            .collect::<Result<Vec<()>>>()?;
        Ok(())
    }

    pub fn stop(self) {
        trace!("Flushing kafka producer");

        self.producer.flush(Timeout::Never);

        trace!("Done flushing kafka producer");
    }
}
