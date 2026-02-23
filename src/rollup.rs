//! Standalone rollup process that owns the rollup MDBX database exclusively.
//!
//! Receives host chain notifications over a Unix domain socket from the
//! ExEx forwarder, processes Signet blocks through the EVM, maintains the
//! rollup state, and serves RPC.

use crate::{
    ipc::{
        CommitPayload, NodeMessage, RevertPayload, RollupResponse, WireBlock, WireReceipt,
    },
    remote_oracle::RemoteAliasOracleFactory,
};
use alloy::{
    consensus::{Block, BlockBody, BlockHeader, SimpleCoder},
    eips::eip2718::Decodable2718,
};
use eyre::Context;
use futures_util::{SinkExt, StreamExt};
use init4_bin_base::utils::from_env::FromEnv;
use reth::{
    primitives::{EthPrimitives, Receipt, RecoveredBlock, SealedBlock, TransactionSigned},
    providers::{
        BlockNumReader, CanonChainTracker, Chain, ExecutionOutcome, HeaderProvider, ProviderFactory,
        providers::BlockchainProvider,
    },
    rpc::types::engine::ForkchoiceState,
    tasks::Runtime,
};
use reth_db::mdbx::DatabaseEnv;
use reth_db_common::init;
use signet_blobber::{BlobFetcher, CacheHandle};
use signet_block_processor::SignetBlockProcessorV1;
use signet_db::{DbProviderExt, ProviderConsistencyExt, RuChain, RuWriter};
use signet_node::GENESIS_JOURNAL_HASH;
use signet_node_config::SignetNodeConfig;
use signet_node_types::SignetNodeTypes;
use signet_rpc::RpcServerGuard;
use signet_types::{PairedHeights, constants::SignetSystemConstants};
use std::{mem::MaybeUninit, path::PathBuf, sync::Arc};
use tokio::{net::UnixListener, sync::watch};
use tracing::{debug, error, info, instrument, warn};

/// Shorthand for the node types used by the rollup process.
///
/// `Arc<DatabaseEnv>` is required because `DatabaseEnv` does not implement
/// `Clone` — `ProviderFactory::new_with_database_path` wraps it in an `Arc`.
type Snt = SignetNodeTypes<DatabaseEnv>;

/// Environment variable for the IPC socket path that the rollup listens on.
const ROLLUP_SOCKET_ENV: &str = "SIGNET_ROLLUP_SOCKET";
/// Default socket path.
const DEFAULT_SOCKET_PATH: &str = "/tmp/signet-rollup.sock";
/// Environment variable for the host chain RPC URL (for alias oracle).
const HOST_RPC_URL_ENV: &str = "SIGNET_HOST_RPC_URL";
/// Default host RPC URL.
const DEFAULT_HOST_RPC_URL: &str = "http://localhost:8545";
/// Default alias cache size.
const ALIAS_CACHE_SIZE: usize = 10_000;

/// Start the standalone rollup process.
pub fn run() -> eyre::Result<()> {
    let config = SignetNodeConfig::from_env()?;
    let socket_path = std::env::var(ROLLUP_SOCKET_ENV).unwrap_or(DEFAULT_SOCKET_PATH.to_string());
    let host_rpc_url: reqwest::Url = std::env::var(HOST_RPC_URL_ENV)
        .unwrap_or(DEFAULT_HOST_RPC_URL.to_string())
        .parse()
        .wrap_err("invalid host RPC URL")?;

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .wrap_err("failed to build tokio runtime")?;

    rt.block_on(run_inner(config, socket_path.into(), host_rpc_url))
}

/// Inner async entry point for the rollup process.
async fn run_inner(
    config: SignetNodeConfig,
    socket_path: PathBuf,
    host_rpc_url: reqwest::Url,
) -> eyre::Result<()> {
    info!("signet rollup process starting");

    let constants = config.constants().wrap_err("failed to load signet constants")?;
    let chain_spec: Arc<_> = config.chain_spec().clone();

    // Task runtime
    let runtime = Runtime::with_existing_handle(tokio::runtime::Handle::current())?;

    // Open the rollup database
    let factory = open_rollup_db(&config)?;

    // Check and fix DB consistency
    if let Some(height) = factory.ru_check_consistency()? {
        unwind_to(&factory, height).wrap_err("failed to unwind RU database")?;
    }

    let bp: BlockchainProvider<Snt> = BlockchainProvider::new(factory.clone())?;

    // HTTP client for blob fetching and alias oracle
    let client = crate::CLIENT.clone();

    // Alias oracle: queries host chain via RPC
    let alias_oracle =
        RemoteAliasOracleFactory::new(host_rpc_url, client.clone(), ALIAS_CACHE_SIZE);

    // Blob fetcher (pool-less mode: only uses explorer, CL, pylon)
    let blob_cacher: CacheHandle = BlobFetcher::builder()
        .with_config(config.block_extractor())?
        .with_client(client.clone())
        .build_cache_no_pool()
        .wrap_err("failed to build blob cacher")?
        .spawn_no_pool::<SimpleCoder>();

    // Block processor
    let processor = SignetBlockProcessorV1::new(
        constants.clone(),
        chain_spec,
        factory.clone(),
        alias_oracle,
        config.slot_calculator(),
        blob_cacher,
    );

    // Status channel
    let (status_tx, _status_rx) = watch::channel(signet_node_types::NodeStatus::Booting);

    // Start the RPC server
    let _rpc_guard = start_rpc(&config, &constants, &bp, &runtime, client.clone()).await?;

    // Determine last processed rollup block
    let last_rollup_block: u64 = factory.last_block_number()?;
    info!(last_rollup_block, "resuming execution from last rollup block found");
    status_tx.send_modify(|s| *s = signet_node_types::NodeStatus::AtHeight(last_rollup_block));

    // Clean up any existing socket file
    let _ = std::fs::remove_file(&socket_path);

    // Bind the listener
    let listener = UnixListener::bind(&socket_path)
        .wrap_err_with(|| format!("failed to bind unix socket at {}", socket_path.display()))?;
    info!(socket = %socket_path.display(), "rollup process listening for connections");

    // Accept connections (one at a time, since there's only one ExEx forwarder)
    loop {
        let (stream, _) = listener
            .accept()
            .await
            .wrap_err("failed to accept connection")?;
        info!("accepted connection from ExEx forwarder");

        let result = handle_connection(
            stream,
            &factory,
            &bp,
            &processor,
            &constants,
            &status_tx,
        )
        .await;

        match result {
            Ok(()) => {
                info!("connection closed gracefully (shutdown)");
                break;
            }
            Err(e) => {
                warn!(%e, "connection lost, waiting for reconnection");
            }
        }
    }

    info!("signet rollup process shutting down");
    Ok(())
}

/// Handle a single connection from the ExEx forwarder.
async fn handle_connection(
    stream: tokio::net::UnixStream,
    factory: &ProviderFactory<Snt>,
    bp: &BlockchainProvider<Snt>,
    processor: &SignetBlockProcessorV1<DatabaseEnv, RemoteAliasOracleFactory>,
    constants: &SignetSystemConstants,
    status_tx: &watch::Sender<signet_node_types::NodeStatus>,
) -> eyre::Result<()> {
    let mut transport = crate::ipc::framed(stream);

    while let Some(frame) = transport.next().await {
        let frame = frame.wrap_err("failed to read frame from forwarder")?;
        let msg: NodeMessage = crate::ipc::decode_message(&frame)?;

        match msg {
            NodeMessage::Notification {
                reverted,
                committed,
                host_safe_block,
                host_finalized_block,
            } => {
                let response = process_notification(
                    factory,
                    bp,
                    processor,
                    constants,
                    status_tx,
                    reverted,
                    committed,
                    host_safe_block,
                    host_finalized_block,
                )
                .await;

                let reply = match response {
                    Ok(finished_host_height) => {
                        RollupResponse::Processed { finished_host_height }
                    }
                    Err(e) => {
                        error!(%e, "error processing notification");
                        RollupResponse::Error(format!("{e:#}"))
                    }
                };

                let encoded = crate::ipc::encode_message(&reply)?;
                transport
                    .send(encoded)
                    .await
                    .wrap_err("failed to send response to forwarder")?;
            }
            NodeMessage::Shutdown => {
                info!("received shutdown message from forwarder");
                let reply = RollupResponse::Processed { finished_host_height: None };
                let encoded = crate::ipc::encode_message(&reply)?;
                let _ = transport.send(encoded).await;
                return Ok(());
            }
        }
    }

    Err(eyre::eyre!("forwarder disconnected"))
}

/// Process a single notification from the forwarder.
#[instrument(skip_all, fields(
    reverted = reverted.is_some(),
    committed = committed.as_ref().map(|c| c.blocks.len()).unwrap_or(0),
))]
async fn process_notification(
    factory: &ProviderFactory<Snt>,
    bp: &BlockchainProvider<Snt>,
    processor: &SignetBlockProcessorV1<DatabaseEnv, RemoteAliasOracleFactory>,
    constants: &SignetSystemConstants,
    status_tx: &watch::Sender<signet_node_types::NodeStatus>,
    reverted: Option<RevertPayload>,
    committed: Option<CommitPayload>,
    host_safe_block: Option<u64>,
    host_finalized_block: Option<u64>,
) -> eyre::Result<Option<u64>> {
    // NB: REVERTS MUST RUN FIRST
    let mut reverted_chain = None;
    if let Some(revert) = reverted {
        reverted_chain = handle_revert(factory, constants, &revert)?;
    }

    let mut committed_chain = None;
    if let Some(commit) = committed {
        let chain = reconstruct_chain(&commit)?;
        committed_chain = processor
            .process_chain(&chain)
            .await
            .wrap_err("error processing committed chain")?;
    }

    // Update canonical state if anything changed
    let finished_host_height = if committed_chain.is_some() || reverted_chain.is_some() {
        update_status(
            factory,
            bp,
            constants,
            status_tx,
            committed_chain,
            reverted_chain,
            host_safe_block,
            host_finalized_block,
        )?
    } else {
        None
    };

    Ok(finished_host_height)
}

/// Handle a host chain revert.
fn handle_revert(
    factory: &ProviderFactory<Snt>,
    constants: &SignetSystemConstants,
    revert: &RevertPayload,
) -> eyre::Result<Option<RuChain>> {
    // If the revert tip is before RU genesis, nothing to do
    if revert.tip_block_number <= constants.host_deploy_height() {
        return Ok(None);
    }

    let target = constants
        .host_block_to_rollup_block_num(revert.first_block_number)
        .unwrap_or_default()
        .saturating_sub(1);

    unwind_to(factory, target).map(Some)
}

/// Unwind the RU chain DB to the target block number.
fn unwind_to(
    factory: &ProviderFactory<Snt>,
    target: u64,
) -> eyre::Result<RuChain> {
    let mut reverted = MaybeUninit::uninit();
    factory
        .provider_rw()?
        .update(|writer| {
            reverted.write(writer.ru_take_blocks_and_execution_above(target)?);
            Ok(())
        })
        // SAFETY: if the closure above returns Ok, reverted is initialized.
        .map(|_| unsafe { reverted.assume_init() })
        .map_err(Into::into)
}

/// Update canonical heights, status channel, and canon state notifications.
fn update_status(
    factory: &ProviderFactory<Snt>,
    bp: &BlockchainProvider<Snt>,
    constants: &SignetSystemConstants,
    status_tx: &watch::Sender<signet_node_types::NodeStatus>,
    committed: Option<RuChain>,
    reverted: Option<RuChain>,
    host_safe_block: Option<u64>,
    host_finalized_block: Option<u64>,
) -> eyre::Result<Option<u64>> {
    let ru_height = factory.last_block_number()?;

    // Update canonical head
    let latest_header = factory
        .sealed_header(ru_height)?
        .expect("ru db inconsistent: no header for height");
    let latest_hash = latest_header.hash();
    bp.set_canonical_head(latest_header);

    let genesis_hash = factory
        .sealed_header(0)?
        .expect("ru db inconsistent: no genesis header")
        .hash();

    // Calculate safe block
    let safe_heights = calculate_safe_heights(constants, host_safe_block, ru_height);
    let safe_hash = if safe_heights.rollup > 0 {
        let safe_header = factory
            .sealed_header(safe_heights.rollup)?
            .expect("ru db inconsistent: no header for safe height");
        let hash = safe_header.hash();
        if hash != genesis_hash {
            bp.set_safe(safe_header);
        }
        hash
    } else {
        genesis_hash
    };

    // Calculate finalized block
    let finalized_heights =
        calculate_finalized_heights(constants, host_finalized_block, ru_height);
    let finalized_hash = if finalized_heights.host > 0 || finalized_heights.rollup > 0 {
        let finalized_header = factory
            .sealed_header(finalized_heights.rollup)?
            .expect("ru db inconsistent: no header for finalized height");
        let hash = finalized_header.hash();
        bp.set_finalized(finalized_header);
        hash
    } else {
        genesis_hash
    };

    // Update forkchoice
    bp.on_forkchoice_update_received(&ForkchoiceState {
        head_block_hash: latest_hash,
        safe_block_hash: safe_hash,
        finalized_block_hash: finalized_hash,
    });
    debug!(%latest_hash, %safe_hash, %finalized_hash, "updated forkchoice");

    // Emit canon state notifications
    emit_canon_state(bp, committed, reverted);

    // Update status channel
    status_tx.send_modify(|s| *s = signet_node_types::NodeStatus::AtHeight(ru_height));

    // Return the finalized host height for the ExEx FinishedHeight event
    let finished = if finalized_hash != genesis_hash {
        Some(finalized_heights.host)
    } else {
        None
    };

    Ok(finished)
}

/// Calculate safe RU heights from host safe height.
fn calculate_safe_heights(
    constants: &SignetSystemConstants,
    host_safe_block: Option<u64>,
    ru_height: u64,
) -> PairedHeights {
    let Some(host_safe) = host_safe_block else {
        return PairedHeights { host: 0, rollup: 0 };
    };

    match constants.pair_host(host_safe) {
        Some(heights) => {
            if heights.rollup > ru_height {
                PairedHeights {
                    host: constants.rollup_block_to_host_block_num(ru_height),
                    rollup: ru_height,
                }
            } else {
                heights
            }
        }
        None => PairedHeights { host: 0, rollup: 0 },
    }
}

/// Calculate finalized RU heights from host finalized height.
fn calculate_finalized_heights(
    constants: &SignetSystemConstants,
    host_finalized_block: Option<u64>,
    ru_height: u64,
) -> PairedHeights {
    let Some(host_finalized) = host_finalized_block else {
        return PairedHeights { host: 0, rollup: 0 };
    };

    match constants.host_block_to_rollup_block_num(host_finalized) {
        Some(finalized_ru) => {
            if finalized_ru > ru_height {
                constants.pair_ru(ru_height)
            } else {
                constants.pair_ru(finalized_ru)
            }
        }
        None => PairedHeights { host: 0, rollup: 0 },
    }
}

/// Emit canonical state notifications via the blockchain provider.
fn emit_canon_state(
    bp: &BlockchainProvider<Snt>,
    committed: Option<RuChain>,
    reverted: Option<RuChain>,
) {
    use reth::providers::CanonStateNotification;

    let notif = match (committed, reverted) {
        (None, None) => None,
        (None, Some(r)) => Some(CanonStateNotification::Reorg {
            old: Arc::new(r.inner),
            new: Arc::new(Default::default()),
        }),
        (Some(c), None) => Some(CanonStateNotification::Commit {
            new: Arc::new(c.inner),
        }),
        (Some(c), Some(r)) => Some(CanonStateNotification::Reorg {
            old: Arc::new(r.inner),
            new: Arc::new(c.inner),
        }),
    };
    if let Some(notif) = notif {
        bp.canonical_in_memory_state().notify_canon_state(notif);
    }
}

/// Reconstruct a `reth::providers::Chain` from an IPC `CommitPayload`.
fn reconstruct_chain(payload: &CommitPayload) -> eyre::Result<Chain<EthPrimitives>> {
    let mut recovered_blocks = Vec::with_capacity(payload.blocks.len());
    let mut all_receipts: Vec<Vec<Receipt>> = Vec::with_capacity(payload.blocks.len());

    for wire_block in &payload.blocks {
        let (block, receipts) = wire_block_to_recovered(wire_block)?;
        recovered_blocks.push(block);
        all_receipts.push(receipts);
    }

    let first_block = recovered_blocks
        .first()
        .map(|b| b.number())
        .unwrap_or(0);

    // Build an ExecutionOutcome with just receipts (no state diffs).
    let outcome = ExecutionOutcome::new(
        Default::default(), // empty BundleState
        all_receipts,
        first_block,
        Vec::new(), // no requests
    );

    Ok(Chain::new(recovered_blocks, outcome, Default::default()))
}

/// Convert a WireBlock into a RecoveredBlock and receipts.
fn wire_block_to_recovered(
    wb: &WireBlock,
) -> eyre::Result<(RecoveredBlock<Block<TransactionSigned>>, Vec<Receipt>)> {
    // Decode transactions from EIP-2718 bytes
    let transactions: Vec<TransactionSigned> = wb
        .body
        .transactions
        .iter()
        .map(|tx_bytes| {
            TransactionSigned::decode_2718(&mut tx_bytes.as_ref())
                .map_err(|e| eyre::eyre!("failed to decode transaction: {e}"))
        })
        .collect::<eyre::Result<Vec<_>>>()?;

    // Reconstruct withdrawals
    let withdrawals = wb.body.withdrawals.as_ref().map(|ws| {
        ws.iter()
            .map(|w| alloy::eips::eip4895::Withdrawal {
                index: w.index,
                validator_index: w.validator_index,
                address: w.address,
                amount: w.amount,
            })
            .collect::<Vec<_>>()
            .into()
    });

    let body = BlockBody {
        transactions,
        ommers: wb.body.ommers.clone(),
        withdrawals,
    };

    let block = Block { header: wb.header.clone(), body };
    let sealed = SealedBlock::new_unchecked(block, wb.block_hash);
    let recovered = RecoveredBlock::new_sealed(sealed, wb.senders.clone());

    // Convert receipts
    let receipts = wb
        .receipts
        .iter()
        .map(wire_receipt_to_receipt)
        .collect();

    Ok((recovered, receipts))
}

/// Convert a WireReceipt to a reth Receipt.
fn wire_receipt_to_receipt(wr: &WireReceipt) -> Receipt {
    Receipt {
        tx_type: alloy::consensus::TxType::try_from(wr.tx_type).unwrap_or_default(),
        success: wr.success,
        cumulative_gas_used: wr.cumulative_gas_used,
        logs: wr
            .logs
            .iter()
            .map(|wl| alloy::primitives::Log {
                address: wl.address,
                data: alloy::primitives::LogData::new_unchecked(
                    wl.topics.clone(),
                    wl.data.clone(),
                ),
            })
            .collect(),
    }
}

/// Open the rollup database and init genesis if needed.
fn open_rollup_db(
    config: &SignetNodeConfig,
) -> eyre::Result<ProviderFactory<Snt>> {
    let factory = ProviderFactory::new_with_database_path(
        config.database_path(),
        config.chain_spec().clone(),
        Default::default(),
        config.static_file_rw()?,
        config.open_rocks_db()?,
        Runtime::with_existing_handle(tokio::runtime::Handle::current())?,
    )?;

    // Init genesis if needed (same logic as SignetNodeBuilder::prebuild)
    use reth::providers::BlockHashReader;
    use reth_db::transaction::DbTxMut;

    if matches!(
        factory.block_hash(0),
        Ok(None)
            | Err(reth::providers::ProviderError::MissingStaticFileBlock(
                reth::primitives::StaticFileSegment::Headers,
                0
            ))
    ) {
        init::init_genesis(&factory)?;

        factory.provider_rw()?.update(|writer| {
            writer.tx_mut().clear::<reth_db::tables::HashedAccounts>()?;
            writer.tx_mut().clear::<reth_db::tables::HashedStorages>()?;
            writer.tx_mut().clear::<reth_db::tables::AccountsTrie>()?;

            writer.tx_ref().put::<signet_db::JournalHashes>(0, GENESIS_JOURNAL_HASH)?;
            Ok(())
        })?;
    }

    Ok(factory)
}

/// Start the RPC server for the rollup (without host chain components).
async fn start_rpc(
    config: &SignetNodeConfig,
    constants: &SignetSystemConstants,
    bp: &BlockchainProvider<Snt>,
    task_executor: &Runtime,
    client: reqwest::Client,
) -> eyre::Result<RpcServerGuard> {
    use signet_rpc::RpcCtx;
    use signet_tx_cache::TxCache;

    let forwarder =
        config.forward_url().map(|url| TxCache::new_with_client(url, client));
    let eth_config = reth::rpc::server_types::eth::EthConfig::default();

    // Create RPC context without host chain components
    let ctx: RpcCtx<(), _> = RpcCtx::new_standalone(
        constants.clone(),
        bp.clone(),
        eth_config,
        forwarder,
        task_executor.clone(),
    )?;

    let router = signet_rpc::router_standalone().with_state(ctx);

    // Build serve config directly (standalone mode: no Reth RpcServerArgs available)
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    let bind_addr = IpAddr::V4(Ipv4Addr::UNSPECIFIED);
    let serve_config = signet_rpc::ServeConfig {
        http: vec![SocketAddr::new(bind_addr, config.http_port())],
        http_cors: None,
        ws: vec![SocketAddr::new(bind_addr, config.ws_port())],
        ws_cors: None,
        ipc: config.ipc_endpoint().map(ToOwned::to_owned),
    };
    serve_config.serve(task_executor, router).await
}
