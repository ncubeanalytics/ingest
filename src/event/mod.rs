use bytes::Bytes;

use crate::{error::Result, kafka::Kafka};

pub async fn forward_to_kafka(
    data: Vec<Bytes>,
    headers: Vec<(&str, &[u8])>,
    kafka: Kafka,
    topic: &str,
) -> Result<()> {
    kafka.send_many(data, headers, topic).await?;
    Ok(())
}
