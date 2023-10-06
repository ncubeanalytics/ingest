use std::collections::HashMap;
use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;

use actix_web::http::Method;
use actix_web::middleware::Condition;
use actix_web::web::PayloadConfig;
use actix_web::{dev::ServerHandle, web, App, HttpRequest, HttpResponse, HttpServer};
use common::config::ConfigError;
use pyo3::{Py, PyAny};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};
use vec1::Vec1;

// pub use connection::ws::WSError;
use state::ServerState;

use crate::config::SchemaConfig;
use crate::python::{import_and_call_callable, init_python};
use crate::{error::Error, error::Result, kafka::Kafka, Config};

mod connection;
mod state;

pub struct Server {
    server_handle: ServerHandle,
    server_task_handle: JoinHandle<std::io::Result<()>>,
    kafka: Kafka,
    // state: web::Data<ServerState>,
    bound_addrs: Vec<SocketAddr>,
}

fn validate_convert_method(s: String) -> std::result::Result<String, String> {
    let m = s.to_ascii_uppercase();
    Method::from_str(&m)
        .map(|_| m)
        .map_err(|_| format!("{} is not a valid http method", s))
}

impl Server {
    pub async fn start(config: Config) -> Result<Self> {
        let mut schema_configs: HashMap<String, SchemaConfig> =
            HashMap::with_capacity(config.service.schema_config.len());
        let mut python_processors: HashMap<String, Py<PyAny>> =
            HashMap::with_capacity(config.service.schema_config.len());
        let mut default_schema_config = config.service.default_schema_config.clone();
        let mut methods_cleaned: Vec<String> = vec![];
        for method in default_schema_config.allowed_methods {
            methods_cleaned
                .push(validate_convert_method(method).map_err(|s| ConfigError::Invalid(s))?)
        }
        default_schema_config.allowed_methods = Vec1::try_from_vec(methods_cleaned).unwrap();
        let mut python_initialized = false;

        let default_python_processor = if let Some(ref python_callable_path) =
            default_schema_config.python_request_processor
        {
            let parts: Vec<&str> = python_callable_path.split(":").collect();
            if parts.len() != 2 {
                return Err(Error::from(ConfigError::Invalid(format!(
                    "Python request processor location should be of the format \
                                <module-dot-path>:<callable object>, found {} instead",
                    python_callable_path
                ))));
            }
            if !python_initialized {
                init_python(&config.service.python_plugin_src_dir)?;
                python_initialized = true;
            }
            debug!(
                "Instantiating python object from {} ...",
                python_callable_path
            );
            Some(import_and_call_callable(parts[0], parts[1])?)
        } else {
            None
        };

        // construct the full schema configs filling in from default values
        for c in config.service.schema_config.iter() {
            let allowed_methods = if let Some(allowed_methods) = &c.schema_config.allowed_methods {
                let mut methods_cleaned: Vec<String> = vec![];
                for method in allowed_methods {
                    methods_cleaned.push(
                        validate_convert_method(method.clone())
                            .map_err(|s| ConfigError::Invalid(s))?,
                    )
                }
                Vec1::try_from_vec(methods_cleaned).unwrap()
            } else {
                default_schema_config.allowed_methods.clone()
            };

            let schema_config = SchemaConfig {
                content_type_from_header: c
                    .schema_config
                    .content_type_from_header
                    .unwrap_or(default_schema_config.content_type_from_header),
                content_type: c
                    .schema_config
                    .content_type
                    .clone()
                    .map(|ct| Some(ct))
                    .unwrap_or(default_schema_config.content_type.clone()),
                forward_request_url: c
                    .schema_config
                    .forward_request_url
                    .unwrap_or(default_schema_config.forward_request_url),
                forward_request_method: c
                    .schema_config
                    .forward_request_method
                    .unwrap_or(default_schema_config.forward_request_method),
                forward_request_http_headers: c
                    .schema_config
                    .forward_request_http_headers
                    .unwrap_or(default_schema_config.forward_request_http_headers),
                response_status: c
                    .schema_config
                    .response_status
                    .unwrap_or(default_schema_config.response_status),
                allowed_methods,
                destination_topic: c
                    .schema_config
                    .destination_topic
                    .clone()
                    .unwrap_or(default_schema_config.destination_topic.clone()),
                python_request_processor: c
                    .schema_config
                    .python_request_processor
                    .clone()
                    .or(default_schema_config.python_request_processor.clone()),
            };
            if schema_configs
                .insert(c.schema_id.clone(), schema_config)
                .is_some()
            {
                return Err(Error::from(ConfigError::Invalid(format!(
                    "Schema with id {} specified more than once in configuration",
                    c.schema_id
                ))));
            }
            if let Some(ref python_callable_path) = c.schema_config.python_request_processor {
                let parts: Vec<&str> = python_callable_path.split(":").collect();
                if parts.len() != 2 {
                    return Err(Error::from(ConfigError::Invalid(format!(
                        "Python request processor location should be of the format \
                                    <module-dot-path>:<callable object>, found {} instead",
                        python_callable_path
                    ))));
                }
                if !python_initialized {
                    init_python(&config.service.python_plugin_src_dir)?;
                    python_initialized = true;
                }
                debug!(
                    "Instantiating python object from {} ...",
                    python_callable_path
                );
                let processor_instance = import_and_call_callable(parts[0], parts[1])?;
                python_processors.insert(c.schema_id.clone(), processor_instance);
            }
        }

        let kafka = Kafka::start(&config)?;

        let state = web::Data::new(ServerState::new(
            kafka.clone(),
            config.headers,
            default_schema_config,
            schema_configs,
            default_python_processor,
            python_processors,
        ));
        let app_state = state.clone();

        let http_server = HttpServer::new(move || {
            let state = app_state.clone();

            App::new()
                .app_data(state)
                .wrap(tracing_actix_web::TracingLogger::default())
                .wrap(Condition::new(
                    config.logging.otel_metrics,
                    actix_web_opentelemetry::RequestMetrics::default(),
                ))
                .service(
                    web::resource("/{schema_id}")
                        .app_data(PayloadConfig::new(
                            config.service.http_payload_limit as usize,
                        ))
                        .route(web::route().to(connection::http::handle)),
                )
                // .service(web::resource("/ws").route(web::get().to(connection::ws::handle)))
                .default_service(web::route().to(|| HttpResponse::NotFound()))
        })
        .disable_signals()
        .keep_alive(Duration::from_secs(config.service.keepalive_seconds))
        .bind(&config.service.addr)?;

        // in case we bind to any available port
        let bound_addrs = http_server.addrs();
        info!("Server listening at {:?}", bound_addrs);
        let server = http_server.run();
        let server_handle = server.handle();
        let server_task_handle = tokio::spawn(server);

        Ok(Server {
            server_handle,
            server_task_handle,
            kafka,
            bound_addrs,
            // state,
        })
    }

    /// Will gracefully stop the server.
    pub async fn stop(self) {
        // debug!("Closing all WebSocket connections");
        // self.state.close_all_ws().await;

        info!("Stopping web server");
        // true means gracefully
        self.server_handle.stop(true).await;

        if let Err(err) = self.server_task_handle.await {
            error!(%err, "Error joining server task");
        }

        info!("Stopping kafka producer");
        self.kafka.stop();
    }

    /// Will ungracefully shut the server down.
    /// Use `stop` for graceful shutdown.
    pub async fn kill(self) {
        warn!("Killing server");
        self.server_handle.stop(false).await;
    }

    pub fn addrs(&self) -> &[SocketAddr] {
        &self.bound_addrs
    }
}

fn get_tenant_id(_req: &HttpRequest) -> i64 {
    // TODO: figure out what this should be. (was: implement this function when authentication is ready)
    1
}
