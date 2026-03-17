//! Kafka producer wrapper.

use bytes::Bytes;
use common::config::ConfigError;
use rdkafka::error::KafkaError;
use rdkafka::message::{DeliveryResult, Header, OwnedHeaders};
use rdkafka::producer::{BaseRecord, ProducerContext, ThreadedProducer};
use rdkafka::{ClientConfig, ClientContext, producer::Producer, util::Timeout};
use std::collections::HashMap;
use std::fs;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{error, instrument, trace};

use crate::config::Config;
use crate::error::{Error, Result};

struct ProducerCtx;

impl ClientContext for ProducerCtx {}

impl ProducerContext for ProducerCtx {
    type DeliveryOpaque = Box<mpsc::Sender<std::result::Result<usize, KafkaError>>>;

    fn delivery(
        &self,
        delivery_result: &DeliveryResult<'_>,
        delivery_opaque: Self::DeliveryOpaque,
    ) {
        let delivery_message = match delivery_result {
            Ok(msg) => Ok(msg.payload_len()),
            Err(e) => Err(e.0.clone()),
        };
        if delivery_opaque.blocking_send(delivery_message).is_err() {
            panic!("Could not send delivery result, the delivery result channel has been closed")
        }
    }
}

type KafkaProducer = ThreadedProducer<ProducerCtx>;

/// Kafka producer wrapper.
/// Can be cheaply cloned.
#[derive(Clone)]
pub struct Kafka(Arc<KafkaInner>);

pub struct KafkaInner {
    producers: HashMap<String, KafkaProducer>,
}

impl Kafka {
    pub fn start(config: &Config) -> Result<Kafka> {
        let mut producers: HashMap<String, KafkaProducer> = HashMap::new();
        for librdkafka_config in &config.librdkafka {
            let mut producer_config = ClientConfig::new();

            for (key, value) in &librdkafka_config.config {
                producer_config.set(key, value);
            }

            for (key, path) in &librdkafka_config.config_from_file {
                let value = fs::read_to_string(path)?;
                producer_config.set(key, value);
            }

            let producer = producer_config.create_with_context(ProducerCtx {})?;
            if producers
                .insert(librdkafka_config.name.to_owned(), producer)
                .is_some()
            {
                return Err(Error::from(ConfigError::Invalid(format!(
                    "Librdkafka configuration with name '{}' specified more than once",
                    librdkafka_config.name
                ))));
            }
        }

        Ok(Self(Arc::new(KafkaInner { producers })))
    }

    fn get_producer(&self, name: &str) -> &KafkaProducer {
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
    pub fn send(
        &self,
        data: &[u8],
        headers: &[(String, Bytes)],
        topic: &str,
        producer_name: &str,
        delivery_tx: mpsc::Sender<std::result::Result<usize, KafkaError>>,
    ) -> std::result::Result<(), KafkaError> {
        let mut kafka_headers = OwnedHeaders::new_with_capacity(headers.len());
        for (key, val) in headers {
            kafka_headers = kafka_headers.insert(Header {
                key,
                value: Some(val.as_ref()),
            });
        }

        let record = BaseRecord::with_opaque_to(topic, Box::new(delivery_tx))
            .payload(data)
            .headers(kafka_headers)
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
            ;

        self.get_producer(producer_name)
            .send(record)
            .map_err(|(error, _)| error)?;
        trace!(topic, "Message successfully sent to kafka broker");
        Ok(())
    }

    pub fn stop(self) {
        trace!("Flushing kafka producers");

        for (name, producer) in self.0.producers.iter() {
            if let Err(e) = producer.flush(Timeout::After(Duration::from_secs(30))) {
                error!("Flushing kafka producer '{}' failed with error {}", name, e);
            }
        }

        trace!("Done flushing kafka producers");
    }
}
