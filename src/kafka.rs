//! Utilities to work with kafka producers.

use bytes::Bytes;
use futures::FutureExt;
use rdkafka::{
    error::{KafkaError, KafkaResult},
    producer::{FutureProducer, FutureRecord},
    util::Timeout,
    ClientConfig,
};

use crate::config::Kafka;

pub fn new_producer(config: &Kafka) -> KafkaResult<FutureProducer> {
    ClientConfig::new()
        .set("bootstrap.servers", &config.servers)
        .set("delivery.timeout.ms", &config.timeout_ms)
        .set("acks", &config.acks)
        .create()
}

pub async fn send(producer: &FutureProducer, data: Bytes, config: &Kafka) -> KafkaResult<()> {
    let record = FutureRecord::to(&config.topic)
        .key("")
        .payload(data.as_ref());

    producer
        .send(record, -1)
        .map(|delivery_status| {
            match delivery_status {
                Err(_) => {
                    // means that something is wrong with the producer
                    Err(KafkaError::Canceled)
                }

                Ok(Err((e, _))) => Err(e),

                _ => Ok(()),
            }
        })
        .await
}

pub fn stop_producer(producer: &FutureProducer) {
    producer.flush(Timeout::Never);
}
