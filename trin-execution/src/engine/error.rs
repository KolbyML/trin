use jsonrpsee::types::{ErrorObject, ErrorObjectOwned};

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum EngineApiError {
    /// Invalid JSON was received by the server.
    #[error("Parse error")]
    ParseError,

    /// The JSON sent is not a valid Request object.
    #[error("Invalid Request")]
    InvalidRequest,

    /// The method does not exist / is not available.
    #[error("Method not found")]
    MethodNotFound,

    /// Invalid method parameter(s).
    #[error("Invalid params")]
    InvalidParams,

    /// Internal JSON-RPC error.
    #[error("Internal error")]
    InternalError,

    /// Generic client error while processing request.
    #[error("Server error")]
    ServerError(String),

    /// Payload does not exist / is not available.
    #[error("Unknown payload")]
    UnknownPayload,

    /// Forkchoice state is invalid / inconsistent.
    #[error("Invalid forkchoice state")]
    InvalidForkchoiceState,

    /// Payload attributes are invalid / inconsistent.
    #[error("Invalid payload attributes")]
    InvalidPayloadAttributes,

    /// Number of requested entities is too large.
    #[error("Too large request")]
    TooLargeRequest,

    /// Payload belongs to a fork that is not supported.
    #[error("Unsupported fork")]
    UnsupportedFork,
}

impl From<EngineApiError> for ErrorObjectOwned {
    fn from(error: EngineApiError) -> Self {
        match &error {
            EngineApiError::ParseError => ErrorObject::owned(-32700, error.to_string(), None::<()>),
            EngineApiError::InvalidRequest => {
                ErrorObject::owned(-32600, error.to_string(), None::<()>)
            }
            EngineApiError::MethodNotFound => {
                ErrorObject::owned(-32601, error.to_string(), None::<()>)
            }
            EngineApiError::InvalidParams => {
                ErrorObject::owned(-32602, error.to_string(), None::<()>)
            }
            EngineApiError::InternalError => {
                ErrorObject::owned(-32603, error.to_string(), None::<()>)
            }
            EngineApiError::ServerError(data) => {
                ErrorObject::owned(-32000, error.to_string(), Some(data))
            }
            EngineApiError::UnknownPayload => {
                ErrorObject::owned(-38001, error.to_string(), None::<()>)
            }
            EngineApiError::InvalidForkchoiceState => {
                ErrorObject::owned(-38002, error.to_string(), None::<()>)
            }
            EngineApiError::InvalidPayloadAttributes => {
                ErrorObject::owned(-38003, error.to_string(), None::<()>)
            }
            EngineApiError::TooLargeRequest => {
                ErrorObject::owned(-38004, error.to_string(), None::<()>)
            }
            EngineApiError::UnsupportedFork => {
                ErrorObject::owned(-38005, error.to_string(), None::<()>)
            }
        }
    }
}
