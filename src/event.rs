use anyhow::Result;
use bytes::Bytes;
use futures::future;
use hyper::{Body, Request};
use tokio::sync::{mpsc, oneshot};

use crate::server::kafka::KafkaEvent;

/// Forwards events from a Request body to a receiver.
/// Events are JSON objects.
/// Request body should be an array of events.
pub async fn forward(req: Request<Body>, forward_to: mpsc::Sender<KafkaEvent>) -> Result<()> {
    let body = hyper::body::to_bytes(req.into_body()).await?;

    // check that body is valid JSON array
    let json_data = json::parse(std::str::from_utf8(body.as_ref())?)?;
    if !json_data.is_array() {
        return Err(json::Error::wrong_type("array").into());
    }

    let mut obj_start = 0;
    let mut braces = 0;
    let mut send_results = Vec::with_capacity(json_data.len());

    for i in 0..body.len() {
        match body.get(i) {
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
                    let obj = body.slice(obj_start..=i);

                    send_results.push(send_obj(obj, forward_to.clone()).await?);
                }
            }

            // do nothing for all other chars
            _ => {}
        }
    }

    future::try_join_all(send_results).await?;

    Ok(())
}

/// Sends the object and returns a receiver for the result.
async fn send_obj(
    obj: Bytes,
    mut send_to: mpsc::Sender<KafkaEvent>,
) -> Result<oneshot::Receiver<Result<()>>> {
    let (sender, receiver) = oneshot::channel();

    let event = (obj, sender);

    send_to.send(event).await?;

    Ok(receiver)
}
