use thiserror::Error;

use crate::protocol::constants::PacketTypeId;

#[derive(Error, Debug)]
pub enum Error {
  #[error("invalid packet length: {0}")]
  InvalidPacketLength(u16),
  #[error("invalid state: no header")]
  InvalidStateNoHeader,

  #[error("io: {0}")]
  Io(#[from] std::io::Error),
  #[error("unexpected bytes after payload: {0}")]
  ExtraPayloadBytes(usize),
  #[error("packet type id mismatch: expected `{expected:?}`, found `{found:?}`")]
  PacketTypeIdMismatch {
    expected: PacketTypeId,
    found: PacketTypeId,
  },
  #[error("bin decode: {0}")]
  BinDecode(#[from] flo_util::binary::BinDecodeError),
  #[error("protobuf decode: {0}")]
  ProtoBufDecode(#[from] prost::DecodeError),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
