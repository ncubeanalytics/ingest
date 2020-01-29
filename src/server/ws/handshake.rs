//! Websocket handshake logic for hyper Requests.

use anyhow::Result;
use http::{
    header::{
        self, HeaderValue, CONNECTION, SEC_WEBSOCKET_ACCEPT, SEC_WEBSOCKET_KEY,
        SEC_WEBSOCKET_VERSION, UPGRADE,
    },
    Method, Request, Response, StatusCode, Version,
};
use hyper::Body;
use sha1::{Digest, Sha1};

const WS_CONNECTION: &str = "Upgrade";
const WS_UPGRADE: &str = "websocket";
const WS_VERSION: &str = "13";
const WS_GUID: &[u8] = b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// Will return a Response for the client.
pub async fn handshake(req: &Request<Body>) -> Result<Response<Body>> {
    // method must be GET
    if req.method() != Method::GET {
        return res_bad_req("Request must be GET");
    }

    // HTTP version must be 1.1
    if req.version() != Version::HTTP_11 {
        return res_bad_req("Only HTTP/1.1 supported");
    }

    let headers = req.headers();

    // all these headers must be present with the correct values
    if !header_is(headers.get(CONNECTION), WS_CONNECTION)
        || !header_is(headers.get(UPGRADE), WS_UPGRADE)
        || !header_is(headers.get(SEC_WEBSOCKET_VERSION), WS_VERSION)
    {
        return res_upgrade_required();
    }

    let accept_key = if let Some(key) = headers.get(SEC_WEBSOCKET_KEY) {
        compute_key(key.as_bytes())
    } else {
        return res_bad_req(Body::empty());
    };

    let res = Response::builder()
        .status(StatusCode::SWITCHING_PROTOCOLS)
        .header(UPGRADE, WS_UPGRADE)
        .header(CONNECTION, WS_CONNECTION)
        .header(SEC_WEBSOCKET_ACCEPT, accept_key.as_str())
        .body(Body::empty())?;

    Ok(res)
}

fn res_bad_req<B>(body: B) -> Result<Response<Body>>
where
    B: Into<Body>,
{
    Ok(Response::builder()
        .status(StatusCode::BAD_REQUEST)
        .body(body.into())?)
}

fn res_upgrade_required() -> Result<Response<Body>> {
    Ok(Response::builder()
        .status(StatusCode::UPGRADE_REQUIRED)
        .header(header::SEC_WEBSOCKET_VERSION, WS_VERSION)
        .body("Expected Upgrade to WebSocket version 13".into())?)
}

fn header_is(header: Option<&HeaderValue>, value: &str) -> bool {
    match header {
        Some(h) => {
            let h_value = h.to_str();

            h_value.is_ok() && h_value.unwrap() == value
        }

        _ => false,
    }
}

fn compute_key(key: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.input(key);
    hasher.input(WS_GUID);

    base64::encode(&hasher.result())
}
