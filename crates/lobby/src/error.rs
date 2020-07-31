use bs_diesel_utils::executor::ExecutorError;
use thiserror::Error;
use tonic::Status;

#[derive(Error, Debug)]
pub enum Error {
  #[error("player token expired")]
  PlayerTokenExpired,
  #[error("player not found")]
  PlayerNotFound,
  #[error("game not found")]
  GameNotFound,
  #[error("only games with `Waiting` status are deletable")]
  GameNotDeletable,
  #[error("invalid game data, please re-create")]
  GameDataInvalid,
  #[error("the game you are trying to join is full")]
  GameFull,
  #[error("this map has no player slot")]
  MapHasNoPlayer,
  #[error("you can only join one game at a time")]
  MultiJoin,
  #[error("player not in game")]
  PlayerNotInGame,
  #[error("send to player channel timeout")]
  PlayerChannelSendTimeout,
  #[error("player channel closed")]
  PlayerChannelClosed,
  #[error("invalid player source state")]
  InvalidPlayerSourceState,
  #[error("operation timeout")]
  Timeout(#[from] tokio::time::Elapsed),
  #[error("net: {0}")]
  Net(#[from] flo_net::error::Error),
  #[error("db error: {0}")]
  Db(#[from] bs_diesel_utils::result::DbError),
  #[error("json: {0}")]
  Json(#[from] serde_json::Error),
  #[error("json web token: {0}")]
  JsonWebToken(#[from] jsonwebtoken::errors::Error),
  #[error("proto: {0}")]
  Proto(#[from] s2_grpc_utils::result::Error),
  #[error("gRPC transport: {0}")]
  GrpcTransport(#[from] tonic::transport::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl From<Error> for String {
  fn from(e: Error) -> Self {
    format!("{}", e)
  }
}

impl From<diesel::result::Error> for Error {
  fn from(e: diesel::result::Error) -> Self {
    Self::Db(e.into())
  }
}

impl From<Error> for Status {
  fn from(e: Error) -> Status {
    match e {
      e @ Error::GameNotFound
      | e @ Error::PlayerNotFound
      | e @ Error::MapHasNoPlayer
      | e @ Error::GameFull
      | e @ Error::GameNotDeletable
      | e @ Error::MultiJoin => Status::invalid_argument(e.to_string()),
      e @ Error::PlayerTokenExpired => Status::unauthenticated(e.to_string()),
      Error::JsonWebToken(e) => Status::unauthenticated(e.to_string()),
      e => Status::internal(e.to_string()),
    }
  }
}

impl From<ExecutorError<diesel::result::Error>> for Error {
  fn from(e: ExecutorError<diesel::result::Error>) -> Self {
    match e {
      ExecutorError::Task(e) => Error::Db(e.into()),
      ExecutorError::Executor(e) => e.into(),
    }
  }
}

impl From<ExecutorError<Error>> for Error {
  fn from(e: ExecutorError<Error>) -> Self {
    match e {
      ExecutorError::Task(e) => e,
      ExecutorError::Executor(e) => Error::Db(e),
    }
  }
}