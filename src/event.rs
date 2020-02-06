use bytes::Bytes;
use futures::future::try_join_all;
use rdkafka::producer::FutureProducer;

use crate::{config::Kafka, error::Result, kafka};

mod parser;

use parser::EventObjParser;

/// Parses and forwards events to a kafka producer.
/// Events are JSON objects.
/// Data should be an array of events.
pub async fn forward_to_kafka(buf: Bytes, producer: &FutureProducer, config: &Kafka) -> Result<()> {
    let parser = EventObjParser::new(buf)?;

    try_join_all(parser.map(|event| kafka::send(producer, event, config))).await?;

    Ok(())
}
