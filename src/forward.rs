//! ExEx forwarder — thin ExEx that receives host chain notifications,
//! serializes them into [`NodeMessage`]s, sends them over a Unix domain
//! socket, and waits for acknowledgement from the rollup process.

use crate::ipc::{
    CommitPayload, NodeMessage, RevertPayload, RollupResponse, WireBlock, WireBlockBody, WireLog,
    WireReceipt, WireWithdrawal,
};
use alloy::{
    consensus::BlockHeader,
    eips::{eip2718::Encodable2718, NumHash},
    primitives::Bytes,
};
use eyre::Context;
use futures_util::{SinkExt, StreamExt};
use alloy::consensus::TxReceipt;
use reth::{
    primitives::EthPrimitives,
    providers::{BlockIdReader, HeaderProvider},
};
use reth_exex::{ExExContext, ExExEvent, ExExNotification};
use reth_node_api::{FullNodeComponents, NodeTypes};
use std::path::{Path, PathBuf};
use tokio::net::UnixStream;
use tracing::{debug, error, info, warn};

/// Thin ExEx that forwards host chain notifications to the rollup process
/// over a Unix domain socket.
pub struct ExExForwarder<Host: FullNodeComponents> {
    /// The ExEx context for receiving notifications and sending events.
    ctx: ExExContext<Host>,
    /// Path to the Unix domain socket.
    socket_path: PathBuf,
}

impl<Host: FullNodeComponents> std::fmt::Debug for ExExForwarder<Host> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExExForwarder")
            .field("socket_path", &self.socket_path)
            .finish_non_exhaustive()
    }
}

impl<Host> ExExForwarder<Host>
where
    Host: FullNodeComponents,
    Host::Types: NodeTypes<Primitives = EthPrimitives>,
{
    /// Create a new forwarder.
    pub fn new(ctx: ExExContext<Host>, socket_path: impl Into<PathBuf>) -> Self {
        Self { ctx, socket_path: socket_path.into() }
    }

    /// Main loop: connect to the rollup process, forward notifications,
    /// and handle reconnection on errors.
    pub async fn start(mut self) -> eyre::Result<()> {
        info!(socket = %self.socket_path.display(), "ExEx forwarder starting");

        loop {
            match self.run_connected().await {
                Ok(()) => {
                    info!("ExEx forwarder shutting down gracefully");
                    return Ok(());
                }
                Err(e) => {
                    error!(%e, "connection to rollup process lost, reconnecting in 1s");
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            }
        }
    }

    /// Connect to the rollup socket and process notifications until
    /// disconnection or shutdown.
    async fn run_connected(&mut self) -> eyre::Result<()> {
        let stream = connect_with_retry(&self.socket_path).await?;
        let mut transport = crate::ipc::framed(stream);

        info!(socket = %self.socket_path.display(), "connected to rollup process");

        while let Some(notification) = self.ctx.notifications.next().await {
            let notification =
                notification.wrap_err("error in reth host notifications stream")?;

            // Read host safe/finalized block numbers before converting
            let host_safe_block = self
                .ctx
                .provider()
                .safe_block_number()
                .ok()
                .flatten();
            let host_finalized_block = self
                .ctx
                .provider()
                .finalized_block_number()
                .ok()
                .flatten();

            let msg =
                notification_to_message(&notification, host_safe_block, host_finalized_block);

            // Send the message
            let encoded = crate::ipc::encode_message(&msg)?;
            transport
                .send(encoded)
                .await
                .wrap_err("failed to send message to rollup process")?;

            debug!("sent notification to rollup process, waiting for ack");

            // Wait for ack
            let response_frame = transport
                .next()
                .await
                .ok_or_else(|| eyre::eyre!("rollup process closed connection"))?
                .wrap_err("failed to read response from rollup process")?;

            let response: RollupResponse = crate::ipc::decode_message(&response_frame)?;

            match response {
                RollupResponse::Processed { finished_host_height } => {
                    debug!(?finished_host_height, "rollup process acked notification");

                    // Send FinishedHeight back to reth if the rollup tells us
                    if let Some(height) = finished_host_height {
                        if let Some(header) =
                            self.ctx.provider().sealed_header(height)?
                        {
                            let hash = header.hash();
                            // Use height - 1 to keep the finalized block available for reorgs
                            let adjusted = height.saturating_sub(1);
                            self.ctx.events.send(ExExEvent::FinishedHeight(
                                NumHash { number: adjusted, hash },
                            ))?;
                            debug!(adjusted, "sent FinishedHeight to reth");
                        }
                    }
                }
                RollupResponse::Error(e) => {
                    error!(%e, "rollup process reported error");
                    return Err(eyre::eyre!("rollup process error: {e}"));
                }
            }
        }

        // Notification stream ended — node is shutting down.
        // Send shutdown message (best-effort).
        let _ = transport
            .send(crate::ipc::encode_message(&NodeMessage::Shutdown)?)
            .await;

        Ok(())
    }
}

/// Connect to a Unix socket, retrying with exponential backoff.
async fn connect_with_retry(path: &Path) -> eyre::Result<UnixStream> {
    let mut delay = std::time::Duration::from_millis(100);
    let max_delay = std::time::Duration::from_secs(30);

    loop {
        match UnixStream::connect(path).await {
            Ok(stream) => return Ok(stream),
            Err(e) => {
                warn!(
                    path = %path.display(),
                    delay_ms = delay.as_millis(),
                    %e,
                    "failed to connect to rollup socket, retrying"
                );
                tokio::time::sleep(delay).await;
                delay = (delay * 2).min(max_delay);
            }
        }
    }
}

/// Convert an ExExNotification into a NodeMessage for IPC transport.
fn notification_to_message<P: reth_node_api::NodePrimitives<
    Block = alloy::consensus::Block<reth::primitives::TransactionSigned>,
    Receipt = reth::primitives::Receipt,
>>(
    notification: &ExExNotification<P>,
    host_safe_block: Option<u64>,
    host_finalized_block: Option<u64>,
) -> NodeMessage {
    // Match directly on the enum to avoid trait bounds on helper methods.
    let (reverted_chain, committed_chain) = match notification {
        ExExNotification::ChainCommitted { new } => (None, Some(new.as_ref())),
        ExExNotification::ChainReorged { old, new } => {
            (Some(old.as_ref()), Some(new.as_ref()))
        }
        ExExNotification::ChainReverted { old } => (Some(old.as_ref()), None),
    };

    let reverted = reverted_chain.map(|chain| RevertPayload {
        first_block_number: chain.first().number(),
        tip_block_number: chain.tip().number(),
    });

    let committed = committed_chain.map(|chain| {
        let blocks = chain
            .blocks_and_receipts()
            .map(|(block, receipts)| {
                let header = block.header().clone();
                let block_hash = block.hash();
                let senders = block.senders().to_vec();

                let body = WireBlockBody {
                    transactions: block
                        .body()
                        .transactions
                        .iter()
                        .map(|tx| Bytes::from(tx.encoded_2718()))
                        .collect(),
                    ommers: block.body().ommers.clone(),
                    withdrawals: block.body().withdrawals.as_ref().map(|ws| {
                        ws.iter()
                            .map(|w| WireWithdrawal {
                                index: w.index,
                                validator_index: w.validator_index,
                                address: w.address,
                                amount: w.amount,
                            })
                            .collect()
                    }),
                };

                let wire_receipts = receipts
                    .iter()
                    .map(|r| WireReceipt {
                        tx_type: r.tx_type as u8,
                        success: r.status(),
                        cumulative_gas_used: r.cumulative_gas_used,
                        logs: r.logs.iter().map(WireLog::from).collect(),
                    })
                    .collect();

                WireBlock {
                    header,
                    body,
                    block_hash,
                    senders,
                    receipts: wire_receipts,
                }
            })
            .collect();

        CommitPayload { blocks }
    });

    NodeMessage::Notification {
        reverted,
        committed,
        host_safe_block,
        host_finalized_block,
    }
}
