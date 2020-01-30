use std::sync::Arc;

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

pub fn new_producer(config: &Kafka) -> KafkaResult<FutureProducer> {
    ClientConfig::new()
        .set("bootstrap.servers", &config.servers)
        .set("delivery.timeout.ms", &config.timeout_ms)
        .create()
}

pub async fn start(
    producer: FutureProducer,
    config: Kafka,
    mut events: mpsc::Receiver<hyper::body::Bytes>,
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

async fn handle_event(producer: FutureProducer, event: hyper::body::Bytes, config: Arc<Kafka>) {
    let record = FutureRecord::to(&config.topic)
        .key("")
        .payload(event.as_ref());

    producer
        .send(record, 0)
        .map(|delivery_status| match delivery_status {
            Ok(Err((e, _))) => eprintln!("Error sending kafka record: {}", e),

            Err(_) => eprintln!("Kafka internal channel closed!"),

            _ => {}
        })
        .await;
}
