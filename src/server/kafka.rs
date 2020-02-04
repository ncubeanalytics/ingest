use std::sync::Arc;

use anyhow::Result;
use bytes::Bytes;
use futures::FutureExt;
use rdkafka::{
    error::KafkaResult,
    producer::{FutureProducer, FutureRecord},
    util::Timeout,
    ClientConfig,
};
use tokio::{
    select,
    sync::{mpsc, oneshot},
    task,
};

use super::config::Kafka;

pub type KafkaEvent = (Bytes, oneshot::Sender<Result<()>>);
pub type KafkaEventChan = mpsc::Receiver<KafkaEvent>;

pub fn new_producer(config: &Kafka) -> KafkaResult<FutureProducer> {
    ClientConfig::new()
        .set("bootstrap.servers", &config.servers)
        .set("delivery.timeout.ms", &config.timeout_ms)
        .set("acks", &config.acks)
        .create()
}

pub async fn start(
    producer: FutureProducer,
    config: Kafka,
    mut events: KafkaEventChan,
    mut close: oneshot::Receiver<()>,
) {
    let config = Arc::new(config);

    loop {
        select! {
            Some(event) = events.recv() => {
                task::spawn(
                    handle_event(producer.clone(), event, config.clone())
                );
            }

            _ = &mut close => {
                println!("Flushing kafka messages...");

                producer.flush(Timeout::Never);

                println!("Done flushing kafka messages");

                return;
            }
        }
    }
}

async fn handle_event(producer: FutureProducer, event: KafkaEvent, config: Arc<Kafka>) {
    let (data, respond_to) = event;

    let record = FutureRecord::to(&config.topic)
        .key("")
        .payload(data.as_ref());

    producer
        .send(record, -1)
        .map(|delivery_status| {
            match delivery_status {
                Ok(Err((e, _))) => respond_to.send(Err(e.into())),

                Err(e) => respond_to.send(Err(e.into())),

                _ => respond_to.send(Ok(())),
            }
            .map_err(failed_to_respond)
        })
        .await
        .ok();
}

/// Used when the sender of an event closes the response channel before
/// receiving a response.
fn failed_to_respond(_: Result<()>) {
    eprintln!("Kafka event sender closed without waiting for result.");
}
