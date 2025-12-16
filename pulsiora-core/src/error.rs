use thiserror::Error;

#[derive(Error, Debug)]
pub enum PulsioraError {
    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Execution error: {0}")]
    ExecutionError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("GitHub API error: {0}")]
    GitHubError(String),

    #[error("Pipeline not found: {0}")]
    PipelineNotFound(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),

    #[error("Network error: {0}")]
    NetworkError(String),
}

pub type Result<T> = std::result::Result<T, PulsioraError>;

