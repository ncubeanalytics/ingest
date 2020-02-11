use std::{error::Error as StdError, fmt, io::Error as IOError, str::Utf8Error};

use actix_web::{
    error::ResponseError,
    http::{header, StatusCode},
    HttpResponse,
};
use rdkafka::error::KafkaError;
use tracing::error;
use tracing_subscriber::filter::ParseError as LogParseError;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    JSON(json::Error),
    Utf8(Utf8Error),
    Kafka(KafkaError),
    IO(IOError),
    LogFilterParse(LogParseError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Error::*;

        match self {
            JSON(e) => write!(f, "Invalid JSON: {}", e),
            Utf8(_) => write!(f, "Invalid utf8 content"),
            Kafka(e) => write!(f, "Kafka producer error: {}", e),
            IO(e) => write!(f, "IO error: {}", e),
            LogFilterParse(e) => write!(f, "Invalid log filter directive: {}", e),
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
            LogFilterParse(e) => Some(e),
        }
    }
}

impl ResponseError for Error {
    fn error_response(&self) -> HttpResponse {
        use Error::*;

        HttpResponse::build(self.status_code())
            .set_header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(match self {
                JSON(_) | Utf8(_) => self.to_string(),

                e @ Kafka(_) | e @ IO(_) | e @ LogFilterParse(_) => {
                    error!("Sending 500 response to client; Internal error: {}", e);

                    "Internal server error".to_string()
                }
            })
    }

    fn status_code(&self) -> StatusCode {
        use Error::*;

        match self {
            JSON(_) | Utf8(_) => StatusCode::BAD_REQUEST,
            Kafka(_) | IO(_) | LogFilterParse(_) => StatusCode::INTERNAL_SERVER_ERROR,
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

impl From<LogParseError> for Error {
    fn from(e: LogParseError) -> Error {
        Error::LogFilterParse(e)
    }
}
