use std::collections::HashMap;
use std::fmt;
use std::net::SocketAddr;

use common::config::CommonConfig;
use common::logging::LoggingConfig;
use serde::{Deserialize, Serialize};
use vec1::{vec1, Vec1};

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct HeaderNames {
    pub schema_id: String,
    pub ip: String,
    pub http_url: String,
    pub http_method: String,
    pub http_header_prefix: String,
    pub ingest_version: String,
}

impl Default for HeaderNames {
    fn default() -> HeaderNames {
        HeaderNames {
            schema_id: "ncube-ingest-schema-id".to_owned(),
            ip: "ncube-ingest-ip".to_owned(),
            http_url: "ncube-ingest-http-url".to_owned(),
            http_method: "ncube-ingest-http-method".to_owned(),
            http_header_prefix: "ncube-ingest-http-header-".to_owned(),
            ingest_version: "ncube-ingest-version".to_owned(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ContentType {
    #[serde(rename = "application/json")]
    Json,
    #[serde(rename = "application/jsonlines")]
    Jsonlines,
    #[serde(rename = "application/octet-stream")]
    Binary,
}

impl fmt::Display for ContentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContentType::Json => write!(f, "application/json"),
            ContentType::Jsonlines => write!(f, "application/jsonlines"),
            ContentType::Binary => write!(f, "application/octet-stream"),
        }
    }
}

#[derive(Clone, Default, Debug, Deserialize)]
#[serde(default)]
pub struct KafkaSecrets {
    pub sasl_password_path: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SchemaConfig {
    #[serde(default = "default_content_type_from_header")]
    pub content_type_from_header: bool,
    #[serde(default = "default_content_type")]
    pub content_type: Option<ContentType>,
    #[serde(default = "default_forward_request_url")]
    pub forward_request_url: bool,
    #[serde(default = "default_forward_request_method")]
    pub forward_request_method: bool,
    #[serde(default = "default_forward_request_http_headers")]
    pub forward_request_http_headers: bool,
    #[serde(default = "default_forward_ingest_version")]
    pub forward_ingest_version: bool,
    #[serde(default = "default_response_status")]
    pub response_status: u16,
    #[serde(default = "default_allowed_methods")]
    pub allowed_methods: Vec1<String>,
    pub destination_topic: String,
    #[serde(default)]
    pub python_request_processor: Vec<PythonProcessorConfig>,
    #[serde(default = "default_librdkafka_config_name")]
    pub librdkafka_config: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PythonProcessorConfig {
    #[serde(default)]
    pub methods: Option<Vec1<String>>,
    pub processor: String,
    #[serde(default)]
    pub implements_process_head: bool,
    #[serde(default)]
    pub process_is_blocking: bool,
    #[serde(default)]
    pub process_head_is_blocking: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct PartialSchemaConfig {
    pub content_type_from_header: Option<bool>,
    pub content_type: Option<ContentType>,
    pub forward_request_url: Option<bool>,
    pub forward_request_method: Option<bool>,
    pub forward_request_http_headers: Option<bool>,
    pub forward_ingest_version: Option<bool>,
    pub response_status: Option<u16>,
    pub allowed_methods: Option<Vec1<String>>,
    pub destination_topic: Option<String>,
    pub python_request_processor: Vec<PythonProcessorConfig>,
    pub librdkafka_config: Option<String>,
}

impl Default for PartialSchemaConfig {
    fn default() -> PartialSchemaConfig {
        PartialSchemaConfig {
            content_type_from_header: None,
            content_type: None,
            forward_request_url: None,
            forward_request_method: None,
            forward_request_http_headers: None,
            forward_ingest_version: None,
            allowed_methods: None,
            response_status: None,
            destination_topic: None,
            python_request_processor: Vec::new(),
            librdkafka_config: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct PartialSchemaConfigWithSchemaId {
    pub schema_id: String,
    #[serde(flatten)]
    pub schema_config: PartialSchemaConfig,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ServiceConfig {
    pub addr: SocketAddr,
    #[serde(default = "default_keepalive_seconds")]
    pub keepalive_seconds: u64,
    #[serde(default = "default_max_event_size_bytes")]
    pub max_event_size_bytes: u64,
    #[serde(default = "default_num_workers")]
    pub num_workers: usize,
    #[serde(default = "default_python_plugin_src_dir")]
    pub python_plugin_src_dir: String,
    pub default_schema_config: SchemaConfig,
    #[serde(default)]
    pub schema_config: Vec<PartialSchemaConfigWithSchemaId>,
}

#[derive(Clone, Default, Debug, Deserialize)]
#[serde(default)]
pub struct LibrdkafkaConfig {
    #[serde(default = "default_librdkafka_config_name")]
    pub name: String,
    #[serde(default)]
    pub config: HashMap<String, String>,
    #[serde(default)]
    pub secrets: KafkaSecrets,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    pub service: ServiceConfig,
    #[serde(default)]
    pub headers: HeaderNames,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub librdkafka: Vec1<LibrdkafkaConfig>,
}

impl CommonConfig for Config {
    const CMD_NAME: &'static str = "ingestd";
    const DEFAULT_CONFIG_PATH: &'static str = "/etc/ncube-ingest/ingest.toml";
}

const fn default_keepalive_seconds() -> u64 {
    300
}
const fn default_max_event_size_bytes() -> u64 {
    1 * 1024 * 1024 // 1Mb, kafka default and events hubs limit
}
fn default_num_workers() -> usize {
    num_cpus::get_physical()
}
fn default_python_plugin_src_dir() -> String {
    "/usr/local/src/ingest/python/".to_owned()
}
const fn default_content_type_from_header() -> bool {
    true
}
const fn default_content_type() -> Option<ContentType> {
    None
}
const fn default_forward_request_url() -> bool {
    false
}
const fn default_forward_request_http_headers() -> bool {
    false
}
const fn default_forward_request_method() -> bool {
    false
}
const fn default_response_status() -> u16 {
    200
}
const fn default_forward_ingest_version() -> bool {
    true
}
fn default_allowed_methods() -> Vec1<String> {
    vec1!["POST".to_owned()]
}
fn default_librdkafka_config_name() -> String {
    "main".to_owned()
}
