use actix::{
    fut::WrapFuture, Actor, ActorContext, ActorFuture, Arbiter, AsyncContext, Handler,
    StreamHandler,
};
use actix_http::ws::Item;
use actix_web::{web, HttpRequest, Responder};
use actix_web_actors::ws;
use bytes::{Bytes, BytesMut};
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
    /// Used for continuation messages
    fragment_buf: Option<BytesMut>,
}

impl WSHandler {
    fn new(kafka: Kafka, server_state: web::Data<ServerState>, log_span: Span) -> Self {
        Self {
            kafka,
            server_state,
            log_span,
            fragment_buf: None,
        }
    }

    fn handle_events(&self, ctx: &mut <Self as Actor>::Context, events: Bytes) {
        let fut = forward_to_kafka(events, self.kafka.clone()).in_current_span();

        let span = Span::current();
        let actor_fut = fut.into_actor(self).map(move |result, _, ctx| {
            let _span_guard = span.enter();

            Self::send_response(ctx, result);
        });

        ctx.spawn(actor_fut);
    }

    fn handle_continuation(&mut self, ctx: &mut <Self as Actor>::Context, fragment: Item) {
        match fragment {
            Item::FirstText(b) | Item::FirstBinary(b) => {
                if self.fragment_buf.is_none() {
                    let buf = BytesMut::from(b.as_ref());

                    self.fragment_buf = Some(buf);
                } else {
                    self.protocol_error_close(ctx, "Client sent first continuation fragment twice");
                }
            }

            Item::Continue(b) => {
                if let Some(buf) = self.fragment_buf.as_mut() {
                    buf.extend_from_slice(b.as_ref());
                } else {
                    self.protocol_error_close(
                        ctx,
                        "Client sent continue fragment without starting continuation",
                    )
                }
            }

            Item::Last(b) => {
                let opt_buf = self.fragment_buf.take();

                if let Some(mut buf) = opt_buf {
                    buf.extend_from_slice(b.as_ref());

                    self.handle_events(ctx, buf.into());
                } else {
                    self.protocol_error_close(
                        ctx,
                        "Client sent last fragment without starting continuation",
                    )
                }
            }
        }
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
        &self,
        reason: Option<ws::CloseReason>,
        ctx: &mut <Self as Actor>::Context,
    ) {
        use ws::CloseCode::Normal;

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

        self.stop(ctx);
    }

    fn state_register(&self, ctx: &mut <Self as Actor>::Context) {
        let state = self.server_state.clone();
        let addr = ctx.address();
        let fut = async move { state.register_ws(addr).await }
            .instrument(self.log_span.clone())
            .into_actor(self)
            .map(|result, handler, ctx| {
                let _span_guard = handler.log_span.enter();

                // if client managed to connect while server is shutting down
                // close connection immediately
                if let Err(Error::WSNotAccepted) = result {
                    warn!("Connection opened while server was shutting down. Closing it");
                    Self::internal_close(ctx);
                }
            });

        ctx.wait(fut);
    }

    fn state_unregister(&self, ctx: &mut <Self as Actor>::Context) {
        let state = self.server_state.clone();
        let addr = ctx.address();

        let fut = async move { state.unregister_ws(&addr).await }.in_current_span();

        Arbiter::spawn(fut);
    }

    fn internal_close(ctx: &mut <Self as Actor>::Context) {
        let reason = ws::CloseReason {
            code: ws::CloseCode::Restart,
            description: None,
        };

        ctx.close(Some(reason));

        ctx.stop();
    }

    /// Used when actix didn't catch protocol errors for some reason
    fn protocol_error_close(&self, ctx: &mut <Self as Actor>::Context, desc: &str) {
        error!(
            "Protocol error not handled by actix! {}; Closing connection",
            desc
        );

        let reason = ws::CloseReason {
            code: ws::CloseCode::Protocol,
            description: None,
        };

        ctx.close(Some(reason));

        self.stop(ctx);
    }

    /// Use to unregister websocket actor from app state and stop it.
    fn stop(&self, ctx: &mut <Self as Actor>::Context) {
        self.state_unregister(ctx);

        ctx.stop();
    }
}

type WSMessage = Result<ws::Message, ws::ProtocolError>;

impl Actor for WSHandler {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.state_register(ctx);
    }
}

impl StreamHandler<WSMessage> for WSHandler {
    fn handle(&mut self, msg: WSMessage, ctx: &mut Self::Context) {
        use ws::Message::*;

        let ws_msg_span = logging::ws_msg_span(&self.log_span);
        let _span_guard = ws_msg_span.enter();

        trace!("Got new message");

        match msg {
            Ok(Binary(buf)) => {
                self.handle_events(ctx, buf);
            }

            Ok(Text(buf)) => {
                self.handle_events(ctx, buf.into());
            }

            Ok(Ping(data)) => {
                ctx.pong(&data);
            }

            Ok(Pong(_)) => {}

            Ok(Close(reason)) => {
                self.handle_client_close(reason, ctx);
            }

            Ok(Continuation(fragment)) => {
                self.handle_continuation(ctx, fragment);
            }

            Err(e) => {
                warn!("Closing. Protocol error: {}", e);

                self.stop(ctx);
            }

            _ => unimplemented!(),
        };
    }
}

impl Handler<WSClose> for WSHandler {
    type Result = ();

    fn handle(&mut self, _msg: WSClose, ctx: &mut Self::Context) {
        let _log_guard = self.log_span.enter();

        trace!("Received internal close message. Closing connection");

        Self::internal_close(ctx);
    }
}
