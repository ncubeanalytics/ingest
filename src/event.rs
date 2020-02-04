use anyhow::{anyhow, Result};
use bytes::Bytes;
use hyper::{Body, Request};
use tokio::sync::mpsc;

/// Forwards events from a Request body to a receiver.
/// Events are JSON objects.
/// Request body should be an array of events.
pub async fn forward(req: Request<Body>, mut forward_to: mpsc::Sender<Bytes>) -> Result<()> {
    let body = hyper::body::to_bytes(req.into_body()).await?;

    let mut obj_start = 0;
    let mut obj_end: usize;
    let mut braces = 0;

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

                if braces < 0 {
                    // input is malformed
                    return invalid_json_err();
                }

                if braces == 0 {
                    // this is the end of an object
                    obj_end = i;

                    let obj = body.slice(obj_start..=obj_end);

                    forward_to.send(obj).await?;
                }
            }

            // do nothing for all other chars
            _ => {}
        }
    }

    if braces != 0 {
        // input is malformed but some objects may have been extracted and sent
        // already
        return invalid_json_err();
    }

    Ok(())
}

fn invalid_json_err() -> Result<()> {
    Err(anyhow!("Invalid JSON"))
}
