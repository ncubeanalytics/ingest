use std::{error::Error as StdError, fmt, io::Error as IOError, str::Utf8Error};

use actix_web::{
    error::ResponseError,
    http::{header, StatusCode},
    HttpResponse,
};
use rdkafka::error::KafkaError;
use tracing::{debug, error};

use common::config::ConfigError;
use common::logging::LoggingError;

use crate::server::WSError;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    JSON(json::Error),
    Utf8(Utf8Error),
    Kafka(KafkaError),
    IO(IOError),
    Logging(LoggingError),
    Config(ConfigError),

    /// Used when server is shutting down and no more websocket connections
    /// are accepted.
    WSNotAccepted,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Error::*;

        match self {
            JSON(e) => write!(f, "Invalid JSON: {}", e),
            Utf8(_) => write!(f, "Invalid utf8 content"),
            Kafka(e) => write!(f, "Kafka producer error: {}", e),
            IO(e) => write!(f, "IO error: {}", e),
            Logging(e) => write!(f, "Invalid log filter directive: {}", e),
            Config(e) => write!(f, "Invalid TOML: {}", e),

            WSNotAccepted => write!(
                f,
                "Server shutting down. No more WebSocket connections accepted"
            ),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        use Error::*;

        match self {
            JSON(e) => Some(e),
            Utf8(e) => Some(e),
            Kafka(e) => Some(e),
            IO(e) => Some(e),
            Logging(e) => Some(e),
            Config(e) => Some(e),

            WSNotAccepted => None,
        }
    }
}

impl ResponseError for Error {
    fn error_response(&self) -> HttpResponse {
        use Error::*;

        let status_code = self.status_code();

        HttpResponse::build(status_code)
            .set_header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(match self {
                JSON(_) | Utf8(_) | WSNotAccepted => {
                    debug!(
                        "Sending {} response to client; Client error: {}",
                        status_code, self
                    );

                    self.to_string()
                }

                e @ Kafka(_) | e @ IO(_) | e @ Logging(_) | e @ Config(_) => {
                    error!(
                        "Sending {} response to client; Internal error: {}",
                        status_code, e
                    );

                    "Internal server error".to_string()
                }
            })
    }

    fn status_code(&self) -> StatusCode {
        use Error::*;

        match self {
            JSON(_) | Utf8(_) => StatusCode::BAD_REQUEST,
            Kafka(_) | IO(_) | Logging(_) | Config(_) => StatusCode::INTERNAL_SERVER_ERROR,

            WSNotAccepted => StatusCode::CONFLICT,
        }
    }
}

impl WSError for Error {
    fn message(&self) -> String {
        use Error::*;

        match self {
            e @ JSON(_) | e @ Utf8(_) => {
                debug!(
                    "Sending unsuccessful response to client; Client error: {}",
                    e
                );

                self.to_string()
            }

            e @ Kafka(_) | e @ IO(_) | e @ Logging(_) | e @ Config(_) | e @ WSNotAccepted => {
                error!(
                    "Sending unsuccessful response to client; Internal error: {}",
                    e
                );

                "Internal server error".to_string()
            }
        }
    }
}

impl From<json::Error> for Error {
    fn from(e: json::Error) -> Error {
        Error::JSON(e)
    }
}

impl From<Utf8Error> for Error {
    fn from(e: Utf8Error) -> Error {
        Error::Utf8(e)
    }
}

impl From<KafkaError> for Error {
    fn from(e: KafkaError) -> Error {
        Error::Kafka(e)
    }
}

impl From<IOError> for Error {
    fn from(e: IOError) -> Error {
        Error::IO(e)
    }
}

impl From<LoggingError> for Error {
    fn from(e: LoggingError) -> Error {
        Error::Logging(e)
    }
}

impl From<ConfigError> for Error {
    fn from(e: ConfigError) -> Error {
        Error::Config(e)
    }
}
