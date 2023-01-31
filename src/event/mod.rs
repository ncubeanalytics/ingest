use std::collections::HashMap;

use bytes::Bytes;

use crate::{error::Result, kafka::Kafka};

pub async fn forward_to_kafka(
    data: Vec<Bytes>,
    headers: HashMap<String, String>,
    kafka: Kafka,
) -> Result<()> {
    kafka.send_many(data, headers).await?;
    Ok(())
}
