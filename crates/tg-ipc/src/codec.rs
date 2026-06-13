//! IPC codec constants and helpers.

/// Maximum frame size for LengthDelimitedCodec.
pub const MAX_FRAME_LEN: usize = super::protocol::IPC_FRAME_MAX;

/// Byte order: big-endian (4-byte length prefix).
pub const LENGTH_FIELD_LEN: usize = 4;
