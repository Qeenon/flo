use flo_util::binary::BinDecodeError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
  #[error("stormlib: {0}")]
  Storm(#[from] stormlib::error::StormError),
  #[error("invalid utf8 bytes: {0}")]
  Utf8(#[from] std::str::Utf8Error),
  #[error("read map info: {0}")]
  ReadInfo(BinDecodeError),
  #[error("read map image: {0}")]
  ReadImage(BinDecodeError),
  #[error("read map minimap icons: {0}")]
  ReadMinimapIcons(BinDecodeError),
  #[error("read map trigger strings: {0}")]
  ReadTriggerStrings(BinDecodeError),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;