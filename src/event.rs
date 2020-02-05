use bytes::Bytes;
use rdkafka::{error::KafkaResult, producer::FutureProducer};

use crate::{config::Kafka, error::Result, kafka};

/// Parses and forwards events to a kafka producer.
/// Events are JSON objects.
/// Data should be an array of events.
pub async fn forward_to_kafka(buf: Bytes, producer: &FutureProducer, config: &Kafka) -> Result<()> {
    // check that body is a valid JSON array
    let json_data = json::parse(std::str::from_utf8(buf.as_ref())?)?;
    if !json_data.is_array() {
        Err(json::Error::wrong_type("array"))?;
    }

    let mut obj_start = 0;
    let mut braces = 0;
    let mut send_results = Vec::with_capacity(json_data.len());

    for i in 0..buf.len() {
        match buf.get(i) {
            // this should never happen
            None => break,

            Some(b'{') => {
                if braces == 0 {
                    obj_start = i;
                }

                braces += 1;
            }

            Some(b'}') => {
                braces -= 1;

                if braces == 0 {
                    // this is the end of an object
                    let obj = buf.slice(obj_start..=i);

                    send_results.push(kafka::send(producer, obj, config).await);
                }
            }

            // do nothing for all other chars
            _ => {}
        }
    }

    send_results.into_iter().collect::<KafkaResult<_>>()?;

    Ok(())
}
