use actix::{
    fut::{wrap_future, WrapFuture},
    Actor, ActorContext, ActorFuture, Addr, AsyncContext, Handler, StreamHandler,
};
use actix_web::{web, HttpRequest, Responder};
use actix_web_actors::ws;
use bytes::Bytes;
use tracing::{error, trace, warn, Span};
use tracing_futures::Instrument;

use crate::error::Error;
use crate::event::forward_to_kafka;
use crate::kafka::Kafka;
use crate::logging;
use crate::server::ServerState;

mod close;
mod error;

pub use close::WSClose;
pub use error::WSError;

pub async fn handle(
    req: HttpRequest,
    stream: web::Payload,
    state: web::Data<ServerState>,
) -> impl Responder {
    let log_span = logging::ws_span();

    log_span.in_scope(|| trace!("Upgrading connection to WebSocket"));

    if state.accepting_ws() {
        ws::start(
            WSHandler::new(state.kafka.clone(), state, log_span),
            &req,
            stream,
        )
    } else {
        Err(Error::WSNotAccepted.into())
    }
}

pub struct WSHandler {
    log_span: Span,
    kafka: Kafka,
    server_state: web::Data<ServerState>,
}

impl WSHandler {
    fn new(kafka: Kafka, server_state: web::Data<ServerState>, log_span: Span) -> Self {
        Self {
            kafka,
            server_state,
            log_span,
        }
    }

    fn handle_events(&self, ctx: &mut <Self as Actor>::Context, events: Bytes, log_span: Span) {
        let fut = forward_to_kafka(events, self.kafka.clone()).instrument(log_span.clone());

        let actor_fut = fut.into_actor(self).map(move |result, _, ctx| {
            let _span_guard = log_span.enter();

            Self::send_response(ctx, result);
        });

        ctx.spawn(actor_fut);
    }

    fn send_response<E>(ctx: &mut <Self as Actor>::Context, result: Result<(), E>)
    where
        E: WSError,
    {
        ctx.text(Self::response_json(result))
    }

    fn response_json<T, E>(result: Result<T, E>) -> String
    where
        E: WSError,
    {
        let mut res_obj = json::object! {
            "success" => result.is_ok(),
        };

        if let Err(e) = result {
            if let Err(json_e) = res_obj.insert("error", e.message()) {
                error!(
                    "Impossible error inserting error message into websocket response: {}",
                    json_e
                )
            }
        } else {
            trace!("Sending back successful response");
        }

        res_obj.dump()
    }

    fn handle_client_close(
        reason: Option<ws::CloseReason>,
        ctx: &mut <Self as Actor>::Context,
        log_span: Span,
    ) {
        use ws::CloseCode::Normal;

        let _log_guard = log_span.enter();

        if let Some(reason) = reason {
            match reason.code {
                Normal => trace!(
                    "Connection closed by client normally; Description: {:?}",
                    reason.description
                ),
                code => warn!(
                    "Connection closed by client with code {:?}; Description: {:?}",
                    code, reason.description
                ),
            }
        } else {
            trace!("Connection closed by client. No reason provided");
        }

        ctx.stop()
    }
}

type WSMessage = Result<ws::Message, ws::ProtocolError>;

impl Actor for WSHandler {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let fut = state_register(self.server_state.clone(), ctx.address())
            .instrument(self.log_span.clone());

        ctx.wait(wrap_future(fut));
    }

    fn stopped(&mut self, ctx: &mut Self::Context) {
        let fut = state_unregister(self.server_state.clone(), ctx.address())
            .instrument(self.log_span.clone());

        ctx.wait(wrap_future(fut));
    }
}

impl StreamHandler<WSMessage> for WSHandler {
    fn handle(&mut self, msg: WSMessage, ctx: &mut Self::Context) {
        use ws::Message::*;

        let _ws_span_guard = self.log_span.enter();
        let ws_msg_span = logging::ws_msg_span();

        ws_msg_span.in_scope(|| trace!("Got new message"));

        match msg {
            Ok(Binary(buf)) => {
                self.handle_events(ctx, buf, ws_msg_span);
            }

            Ok(Text(buf)) => {
                self.handle_events(ctx, buf.into(), ws_msg_span);
            }

            Ok(Ping(_)) => {
                ctx.pong(b"pong");
            }

            Ok(Close(reason)) => Self::handle_client_close(reason, ctx, ws_msg_span),

            _ => unimplemented!(),
        };
    }
}

impl Handler<WSClose> for WSHandler {
    type Result = ();

    fn handle(&mut self, _msg: WSClose, ctx: &mut Self::Context) {
        let _log_guard = self.log_span.enter();

        trace!("Received internal close message");

        let reason = ws::CloseReason {
            code: ws::CloseCode::Restart,
            description: None,
        };

        ctx.close(Some(reason));

        ctx.stop();
    }
}

async fn state_register(state: web::Data<ServerState>, addr: Addr<WSHandler>) {
    state.register_ws(addr).await
}

async fn state_unregister(state: web::Data<ServerState>, addr: Addr<WSHandler>) {
    state.unregister_ws(&addr).await
}
