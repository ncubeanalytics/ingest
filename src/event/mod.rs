use bytes::Bytes;

use crate::{error::Result, kafka::Kafka};

/// Parses and forwards events to a kafka producer.
/// Events are JSON objects.
/// Data should be an array of events.
pub async fn forward_to_kafka(data: Bytes, kafka: Kafka, tenant_id: i64) -> Result<()> {
    kafka.send(data, tenant_id).await?;

    Ok(())
}
