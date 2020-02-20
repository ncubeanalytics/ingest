use bytes::Bytes;
use futures::future::try_join_all;

use crate::{error::Result, kafka::Kafka};

mod parser;

use parser::EventObjParser;

/// Parses and forwards events to a kafka producer.
/// Events are JSON objects.
/// Data should be an array of events.
pub async fn forward_to_kafka(buf: Bytes, kafka: Kafka) -> Result<()> {
    let parser = EventObjParser::new(buf)?;

    try_join_all(parser.map(|event| kafka.send(event))).await?;

    Ok(())
}
