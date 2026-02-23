//! IPC message types and framing utilities for communication between the
//! signet ExEx forwarder and the standalone rollup process.
//!
//! Messages are length-delimited bincode frames sent over a Unix domain socket.

use alloy::{
    consensus::Header,
    primitives::{Address, Bytes, B256, Log},
};
use serde::{Deserialize, Serialize};

/// An individual log entry, serialized with only primitive fields for
/// maximum serialization compatibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireLog {
    /// Address that emitted this log.
    pub address: Address,
    /// Indexed topic values.
    pub topics: Vec<B256>,
    /// Non-indexed data.
    pub data: Bytes,
}

impl From<&Log> for WireLog {
    fn from(log: &Log) -> Self {
        Self {
            address: log.address,
            topics: log.topics().to_vec(),
            data: log.data.data.clone(),
        }
    }
}

/// Receipt data for a single transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireReceipt {
    /// EIP-2718 transaction type.
    pub tx_type: u8,
    /// Whether execution succeeded.
    pub success: bool,
    /// Cumulative gas used in the block up to and including this tx.
    pub cumulative_gas_used: u64,
    /// Logs emitted by this transaction.
    pub logs: Vec<WireLog>,
}

/// A single host-chain block with receipts, serialized for IPC transport.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireBlock {
    /// The block header.
    pub header: Header,
    /// The block body (transactions, ommers, withdrawals).
    pub body: WireBlockBody,
    /// The block hash (precomputed).
    pub block_hash: B256,
    /// Recovered transaction senders, one per transaction.
    pub senders: Vec<Address>,
    /// Receipts, one per transaction.
    pub receipts: Vec<WireReceipt>,
}

/// Block body with transactions encoded as EIP-2718 bytes, avoiding
/// complex generic serialization of transaction types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireBlockBody {
    /// Transactions encoded as EIP-2718 envelopes.
    pub transactions: Vec<Bytes>,
    /// Ommer headers (empty post-merge).
    pub ommers: Vec<Header>,
    /// Withdrawals (if present, post-Shanghai).
    pub withdrawals: Option<Vec<WireWithdrawal>>,
}

/// A withdrawal, matching alloy's Withdrawal struct.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct WireWithdrawal {
    /// Monotonically increasing index.
    pub index: u64,
    /// Index of the validator.
    pub validator_index: u64,
    /// Target address for the withdrawn ether.
    pub address: Address,
    /// Value of the withdrawal in gwei.
    pub amount: u64,
}

/// Payload for a committed host chain segment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitPayload {
    /// Blocks in the committed chain, ordered by block number.
    pub blocks: Vec<WireBlock>,
}

/// Payload for a reverted host chain segment.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RevertPayload {
    /// Block number of the first reverted block.
    pub first_block_number: u64,
    /// Block number of the tip (last) reverted block.
    pub tip_block_number: u64,
}

/// Message sent from the ExEx forwarder (signet node) to the rollup process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeMessage {
    /// An ExEx notification, converted for IPC transport.
    Notification {
        /// Reverted chain info (if any). Only contains block number range.
        reverted: Option<RevertPayload>,
        /// Committed chain blocks and receipts (if any).
        committed: Option<CommitPayload>,
        /// The host chain's safe block number, if known.
        host_safe_block: Option<u64>,
        /// The host chain's finalized block number, if known.
        host_finalized_block: Option<u64>,
    },
    /// Graceful shutdown request.
    Shutdown,
}

/// Response sent from the rollup process back to the ExEx forwarder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RollupResponse {
    /// Successfully processed the notification.
    Processed {
        /// The highest finalized host block height, used for ExExEvent::FinishedHeight.
        finished_host_height: Option<u64>,
    },
    /// An error occurred during processing.
    Error(String),
}

// --- Serialization helpers ---

/// Serialize a message to bincode bytes.
pub fn serialize<T: Serialize>(msg: &T) -> eyre::Result<Vec<u8>> {
    bincode::serialize(msg).map_err(|e| eyre::eyre!("bincode serialize error: {e}"))
}

/// Deserialize a message from bincode bytes.
pub fn deserialize<'a, T: Deserialize<'a>>(bytes: &'a [u8]) -> eyre::Result<T> {
    bincode::deserialize(bytes).map_err(|e| eyre::eyre!("bincode deserialize error: {e}"))
}

// --- Framing utilities ---

use bytes::BytesMut;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

/// Create a length-delimited framed transport over any async read/write stream.
///
/// Uses a 4-byte big-endian length prefix. Max frame size is 256 MiB
/// to handle large chain segments during initial sync.
pub fn framed<T: AsyncRead + AsyncWrite>(io: T) -> Framed<T, LengthDelimitedCodec> {
    LengthDelimitedCodec::builder()
        .max_frame_length(256 * 1024 * 1024) // 256 MiB
        .new_framed(io)
}

/// Encode a serializable message into a BytesMut suitable for sending
/// through the framed transport.
pub fn encode_message<T: Serialize>(msg: &T) -> eyre::Result<bytes::Bytes> {
    let bytes = serialize(msg)?;
    Ok(bytes::Bytes::from(bytes))
}

/// Decode a message from a BytesMut received from the framed transport.
pub fn decode_message<'a, T: Deserialize<'a>>(buf: &'a BytesMut) -> eyre::Result<T> {
    deserialize(buf)
}
