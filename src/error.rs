use std::{error::Error as StdError, fmt, io::Error as IOError};

use actix_web::{error::ResponseError, http::StatusCode, HttpResponse};
use rdkafka::error::KafkaError;
use serde::Serialize;
use tracing::{debug, error};

use common::config::ConfigError;
use common::logging::LoggingError;

use crate::server::WSError;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
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
            Kafka(e) => Some(e),
            IO(e) => Some(e),
            Logging(e) => Some(e),
            Config(e) => Some(e),

            WSNotAccepted => None,
        }
    }
}

#[derive(Serialize)]
struct JSONError {
    error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
}

impl From<&Error> for JSONError {
    fn from(e: &Error) -> JSONError {
        use Error::*;

        let (error, description) = match e {
            WSNotAccepted => ("ws_not_accepted".to_string(), Some(e.to_string())),

            // internal server errors should not be converted to JSONError
            Kafka(_) | IO(_) | Logging(_) | Config(_) => ("".to_string(), None),
        };

        JSONError { error, description }
    }
}

impl ResponseError for Error {
    fn error_response(&self) -> HttpResponse {
        use Error::*;

        let status_code = self.status_code();

        let mut res = HttpResponse::build(status_code);

        match self {
            WSNotAccepted => {
                debug!(
                    "Sending {} response to client; Client error: {}",
                    status_code, self
                );

                res.json(JSONError::from(self))
            }

            Kafka(_) | IO(_) | Logging(_) | Config(_) => {
                error!(
                    "Sending {} response to client; Internal error: {}",
                    status_code, self
                );

                res.finish()
            }
        }
    }

    fn status_code(&self) -> StatusCode {
        use Error::*;

        match self {
            Kafka(_) | IO(_) | Logging(_) | Config(_) => StatusCode::INTERNAL_SERVER_ERROR,

            WSNotAccepted => StatusCode::CONFLICT,
        }
    }
}

impl WSError for Error {
    fn message(&self) -> String {
        use Error::*;

        match self {
            Kafka(_) | IO(_) | Logging(_) | Config(_) | WSNotAccepted => {
                error!(
                    "Sending unsuccessful response to client; Internal error: {}",
                    self
                );

                "Internal server error".to_string()
            }
        }
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
