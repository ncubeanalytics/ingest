use std::collections::HashMap;
use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;

use actix_web::http::Method;
use actix_web::middleware::Condition;
use actix_web::{dev::ServerHandle, web, App, HttpResponse, HttpServer};
use common::config::ConfigError;
use pyo3::{Py, PyAny};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};
use vec1::Vec1;

// pub use connection::ws::WSError;
use state::ServerState;

use crate::config::{PythonProcessorConfig, SchemaConfig};
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

fn validate_convert_method(s: &str) -> std::result::Result<String, String> {
    let m = s.to_ascii_uppercase();
    Method::from_str(&m)
        .map(|_| m)
        .map_err(|_| format!("{} is not a valid http method", s))
}

fn validate_convert_methods(methods: &[String]) -> std::result::Result<Vec1<String>, String> {
    let mut methods_cleaned: Vec<String> = vec![];
    for method in methods {
        methods_cleaned.push(validate_convert_method(method)?)
    }
    methods_cleaned.sort();
    methods_cleaned.dedup();
    Ok(Vec1::try_from_vec(methods_cleaned).unwrap())
}

impl Server {
    pub async fn start(config: Config) -> Result<Self> {
        let kafka = Kafka::start(&config)?;
        let kafka_producer_names = kafka.producer_names();

        let mut schema_configs: HashMap<String, SchemaConfig> =
            HashMap::with_capacity(config.service.schema_config.len());
        let mut python_processor_resolver =
            PythonProcessorResolver::new(&config.service.python_plugin_src_dir);
        let mut default_schema_config = config.service.default_schema_config.clone();
        if !kafka_producer_names.contains(&default_schema_config.librdkafka_config.as_str()) {
            return Err(Error::from(ConfigError::Invalid(format!(
                "Librdkafka config with name '{}' configured on default schema config not found. \
                Available librdkafka configs: {:?}",
                default_schema_config.librdkafka_config, kafka_producer_names
            ))));
        }
        let methods_cleaned = validate_convert_methods(&default_schema_config.allowed_methods)
            .map_err(|s| ConfigError::Invalid(s))?;
        default_schema_config.allowed_methods = methods_cleaned;

        for default_python_processor_config in default_schema_config.python_request_processor.iter()
        {
            python_processor_resolver.add_default(default_python_processor_config)?;
        }

        // construct the full schema configs filling in from default values
        for c in config.service.schema_config.iter() {
            let allowed_methods = if let Some(allowed_methods) = &c.schema_config.allowed_methods {
                validate_convert_methods(&allowed_methods).map_err(|s| ConfigError::Invalid(s))?
            } else {
                default_schema_config.allowed_methods.clone()
            };
            let librdkafka_config =
                if let Some(librdkafka_config) = &c.schema_config.librdkafka_config {
                    if !kafka_producer_names.contains(&librdkafka_config.as_str()) {
                        return Err(Error::from(ConfigError::Invalid(format!(
                        "Librdkafka config with name '{}' configured on schema '{}' not found. \
                        Available librdkafka configs: {:?}",
                        librdkafka_config, c.schema_id, kafka_producer_names
                    ))));
                    } else {
                        librdkafka_config.clone()
                    }
                } else {
                    default_schema_config.librdkafka_config.clone()
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
                forward_ingest_version: c
                    .schema_config
                    .forward_ingest_version
                    .unwrap_or(default_schema_config.forward_ingest_version),
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
                python_request_processor: c.schema_config.python_request_processor.clone(),
                librdkafka_config,
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
            for schema_python_processor_config in c.schema_config.python_request_processor.iter() {
                python_processor_resolver
                    .add_for_schema(&c.schema_id, schema_python_processor_config)?;
            }
        }

        let state = web::Data::new(ServerState {
            kafka: kafka.clone(),
            header_names: config.headers,
            default_schema_config,
            schema_configs,
            python_processor_resolver,
            max_event_size_bytes: config.service.max_event_size_bytes,
        });
        let app_state = state.clone();

        let http_server = HttpServer::new(move || {
            let state = app_state.clone();

            App::new()
                .app_data(state)
                .wrap(common::logging::actix_web::tracing_logger())
                .wrap(Condition::new(
                    config.logging.otel_metrics,
                    actix_web_opentelemetry::RequestMetrics::default(),
                ))
                .wrap(Condition::new(
                    config.logging.sentry.enabled,
                    sentry_actix::Sentry::new(),
                ))
                .service(
                    web::resource("/{schema_id}").route(web::route().to(connection::http::handle)),
                )
                .service(
                    web::resource("/{schema_id}/{rest:.*}")
                        .route(web::route().to(connection::http::handle_with_trailing_path)),
                )
                // .service(web::resource("/ws").route(web::get().to(connection::ws::handle)))
                .default_service(web::route().to(|| HttpResponse::NotFound()))
        })
        .disable_signals()
        .keep_alive(Duration::from_secs(config.service.keepalive_seconds))
        .workers(config.service.num_workers)
        .bind(&config.service.address)?;

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

pub struct PythonProcessor {
    processor: Py<PyAny>,
    pub implements_process_head: bool,
    pub process_is_blocking: bool,
    pub process_head_is_blocking: bool,
}

pub struct PythonProcessorResolver {
    callable_path_to_processor: HashMap<String, PythonProcessor>,
    schema_to_processor: HashMap<String, (Option<String>, HashMap<String, String>)>,
    default_processor: (Option<String>, HashMap<String, String>),
    python_initialized: bool,
    python_plugin_src_dir: String,
}

impl PythonProcessorResolver {
    fn new(python_plugin_src_dir: &str) -> Self {
        Self {
            callable_path_to_processor: HashMap::new(),
            schema_to_processor: HashMap::new(),
            default_processor: (None, HashMap::new()),
            python_initialized: false,
            python_plugin_src_dir: python_plugin_src_dir.to_owned(),
        }
    }

    fn instantiate(&mut self, processor_config: &PythonProcessorConfig) -> Result<String> {
        let python_callable_path = processor_config.processor.clone();
        if self
            .callable_path_to_processor
            .contains_key(&python_callable_path)
        {
            return Ok(python_callable_path);
        }
        let parts: Vec<&str> = python_callable_path.split(":").collect();
        if parts.len() != 2 {
            return Err(Error::from(ConfigError::Invalid(format!(
                "Python request processor location should be of the format \
                                <module-dot-path>:<callable object>, found {} instead",
                python_callable_path
            ))));
        }

        debug!(
            "Instantiating python object from {} ...",
            python_callable_path
        );
        if !self.python_initialized {
            init_python(&self.python_plugin_src_dir)?;
            self.python_initialized = true;
        }
        let python_processor = import_and_call_callable(parts[0], parts[1])?;
        let processor = PythonProcessor {
            processor: python_processor,
            implements_process_head: processor_config.implements_process_head,
            process_is_blocking: processor_config.process_is_blocking,
            process_head_is_blocking: processor_config.process_head_is_blocking,
        };
        self.callable_path_to_processor
            .insert(python_callable_path.clone(), processor);
        Ok(python_callable_path)
    }

    fn add(
        &mut self,
        processor_config: &PythonProcessorConfig,
        schema_id_opt: Option<&str>,
        // default_and_method_specific: &mut (Option<String>, HashMap<String, String>),
    ) -> Result<()> {
        let path = self.instantiate(processor_config)?;
        let default_and_method_specific = if let Some(schema_id) = schema_id_opt {
            self.schema_to_processor
                .entry(schema_id.to_owned())
                .or_insert((None, HashMap::new()))
        } else {
            &mut self.default_processor
        };

        // if it has methods, validate them and fail if they already exist
        // if it does not have methods, fail if the default is already set
        if let Some(methods) = &processor_config.methods {
            let cleaned_methods =
                validate_convert_methods(&methods).map_err(|s| ConfigError::Invalid(s))?;
            for method in cleaned_methods {
                if default_and_method_specific.1.contains_key(&method) {
                    return Err(Error::from(ConfigError::Invalid(format!(
                        "Duplicate method '{}' configured for Python request processor '{}'",
                        method, path
                    ))));
                }
                default_and_method_specific.1.insert(method, path.clone());
            }
        } else {
            if default_and_method_specific.0.is_some() {
                return Err(Error::from(ConfigError::Invalid(format!(
                    "Duplicate Python request processor '{}'",
                    path
                ))));
            }
            default_and_method_specific.0 = Some(path)
        }
        Ok(())
    }

    fn add_default(&mut self, processor_config: &PythonProcessorConfig) -> Result<()> {
        match self.add(
            processor_config,
            None,
            // &mut self.default_processor
        ) {
            Err(Error::Config(ConfigError::Invalid(s))) => Err(Error::Config(
                ConfigError::Invalid("Default Python request processor: ".to_owned() + &s),
            )),
            r @ Ok(_) => r,
            r @ Err(_) => r,
        }
    }
    fn add_for_schema(
        &mut self,
        schema_id: &str,
        processor_config: &PythonProcessorConfig,
    ) -> Result<()> {
        match self.add(processor_config, Some(schema_id)) {
            Err(Error::Config(ConfigError::Invalid(s))) => {
                Err(Error::Config(ConfigError::Invalid(
                    format!("Python request processor for schema {}: ", schema_id) + &s,
                )))
            }
            r @ Ok(_) => r,
            r @ Err(_) => r,
        }
    }

    fn get(&self, schema_id: &str, method: &str) -> Option<&PythonProcessor> {
        // first locate schema specific, otherwise default
        // then locate method specific, otherwise default
        if let Some((default, method_specific)) = self
            .schema_to_processor
            .get(schema_id)
            .or(Some(&self.default_processor))
        {
            if let Some(path) = method_specific.get(method).or(default.as_ref()) {
                return Some(self.callable_path_to_processor.get(path).expect(&format!(
                    "Callable path {} does not exist in paths: {:?}",
                    path,
                    self.callable_path_to_processor.keys().collect::<Vec<&String>>()
                )));
            }
        }
        None
    }
}
