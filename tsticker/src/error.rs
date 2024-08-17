use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error{
    #[error("bot error: {0}")]
    BotError(String),

    #[error(transparent)]
    RequestError(#[from] reqwest::Error),

    #[error(transparent)]
    ResponseJsonError(serde_json::Error)

}


pub type Result<T> = std::result::Result<T, Error>;