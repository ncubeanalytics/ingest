use bytes::Bytes;
use rdkafka::error::KafkaError;

use crate::kafka::Kafka;

#[allow(dead_code)]
pub async fn forward_to_kafka(
    data: Vec<Bytes>,
    headers: Vec<(String, Bytes)>,
    kafka: Kafka,
    topic: &str,
) -> Result<(), KafkaError> {
    kafka.send_many(data, headers, topic).await?;
    Ok(())
}

pub async fn forward_message_to_kafka(
    data: Bytes,
    headers: Vec<(String, Bytes)>,
    kafka: Kafka,
    topic: &str,
) -> Result<usize, KafkaError> {
    let data_len = data.len();
    kafka.send(data, headers, topic).await?;
    Ok(data_len)
}
