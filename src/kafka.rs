//! Kafka producer wrapper.

use std::collections::HashMap;
use std::fs;
use std::sync::Arc;

use bytes::Bytes;
use common::config::ConfigError;
use futures::future::join_all;
use futures::TryFutureExt;
use rdkafka::error::KafkaError;
use rdkafka::message::{Header, OwnedHeaders};
use rdkafka::{
    producer::{FutureProducer, FutureRecord, Producer},
    util::Timeout,
    ClientConfig,
};
use tracing::{error, instrument, trace};

use crate::config::Config;
use crate::error::{Error, Result};

/// Kafka producer wrapper.
/// Can be cheaply cloned.
#[derive(Clone)]
pub struct Kafka(Arc<KafkaInner>);

pub struct KafkaInner {
    producers: HashMap<String, FutureProducer>,
}

impl Kafka {
    pub fn start(config: &Config) -> Result<Kafka> {
        let mut producers: HashMap<String, FutureProducer> = HashMap::new();
        for librdkafka_config in &config.librdkafka {
            let mut producer_config = ClientConfig::new();

            for (key, value) in &librdkafka_config.config {
                producer_config.set(key, value);
            }

            if let Some(sasl_password_path) = &librdkafka_config.secrets.sasl_password_path {
                let content = fs::read_to_string(sasl_password_path)?;
                producer_config.set("sasl.password", content);
            }

            let producer = producer_config.create()?;
            if let Some(_) = producers.insert(librdkafka_config.name.to_owned(), producer) {
                return Err(Error::from(ConfigError::Invalid(format!(
                    "Librdkafka configuration with name '{}' specified more than once",
                    librdkafka_config.name
                ))));
            }
        }

        Ok(Self(Arc::new(KafkaInner { producers })))
    }

    fn get_producer(&self, name: &str) -> &FutureProducer {
        &self.0.producers[name]
    }

    pub fn producer_names(&self) -> Vec<&str> {
        self.0
            .producers
            .keys()
            .map(|s| s.as_str())
            .collect::<Vec<&str>>()
    }

    #[instrument(
        level = "trace",
        name = "send_kafka_message",
        skip(self, data, headers)
    )]
    pub async fn send(
        &self,
        data: Bytes,
        headers: Vec<(String, Bytes)>,
        topic: &str,
        producer_name: &str,
    ) -> std::result::Result<(), KafkaError> {
        let mut kafka_headers = OwnedHeaders::new_with_capacity(headers.len());
        for (key, val) in headers {
            kafka_headers = kafka_headers.insert(Header {
                key: &key,
                value: Some(val.as_ref()),
            });
        }
        let record = FutureRecord::to(topic)
            .key("")
            // an empty key with partitioner:consistent_random will randomly distribute across
            // the partitions
            // XXX: figure out how to specify key
            // options:
            // 1. support passing the partition key as a header in the POST request
            // 2. specify it as a json attribute. would need to parse json
            // 3. how to specify it in newline-delimited json?
            // 4. support adding it in python request processor?
            // 5. should not employ partitioning in actix web workers. (cannot. they are given some
            // kind of connection object before the data is even read). but in general should not
            // if a client is spamming requests for the same partition, client should switch to streaming
            .payload(data.as_ref())
            .headers(kafka_headers);

        self.get_producer(producer_name)
            .send(record, Timeout::Never)
            .map_err(|(error, _)| error)
            .map_ok(|_| {
                trace!(topic, "Message successfully sent to kafka broker");
            })
            .await
    }

    #[allow(dead_code)]
    pub async fn send_many(
        &self,
        data: Vec<Bytes>,
        headers: Vec<(String, Bytes)>,
        topic: &str,
        producer_name: &str,
    ) -> std::result::Result<(), KafkaError> {
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
            futures.push(self.send(d, headers.clone(), topic, producer_name))
        }

        join_all(futures) // XXX: does join_all execute in order?
            .await
            .into_iter()
            .collect::<std::result::Result<Vec<()>, KafkaError>>()?;
        Ok(())
    }

    pub fn stop(self) {
        trace!("Flushing kafka producers");

        for (name, producer) in self.0.producers.iter() {
            // XXX: should add some timeout
            if let Err(e) = producer.flush(Timeout::Never) {
                error!("Flushing kafka producer '{}' failed with error {}", name, e);
            }
        }

        trace!("Done flushing kafka producers");
    }
}
