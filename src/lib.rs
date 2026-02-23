// TODO: ENG-480 - Remove allow dead code macros
// https://linear.app/initiates/issue/ENG-480/remove-allow-dead-code-proc-macros
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![deny(unused_must_use, rust_2018_idioms)]
#![warn(missing_docs, missing_copy_implementations, missing_debug_implementations)]

//! # Signet
//!
//! This crate provides two operational modes:
//!
//! - **Legacy mode** (`node()` / `node_from_env()`): Single-process ExEx that
//!   opens both the host chain and rollup MDBX databases in the same process.
//!
//! - **IPC mode** (`node_with_ipc()` / `rollup::run()`): Two-process
//!   architecture where the signet node runs a thin ExEx forwarder that sends
//!   notifications over a Unix domain socket to a standalone rollup process.
//!   This avoids MDBX corruption from two environments in one process.

use openssl as _;
use reth_chainspec as _;

use init4_bin_base::utils::from_env::FromEnv;
use reth::providers::ProviderFactory;
use signet_node::SignetNodeBuilder;
use signet_node_config::SignetNodeConfig;
use std::sync::{Arc, LazyLock};

pub mod forward;
pub mod ipc;
pub mod remote_oracle;
pub mod rollup;

/// The global reqwest client used throughout the node.
pub static CLIENT: LazyLock<reqwest::Client> =
    LazyLock::new(|| reqwest::Client::builder().use_rustls_tls().build().unwrap());

/// Start the Signet node, reading config from env.
///
/// This uses the legacy single-process mode. For the IPC mode, use
/// [`node_with_ipc_from_env`] instead.
pub fn node_from_env() -> eyre::Result<()> {
    SignetNodeConfig::from_env().map(node)?
}

/// Start the Signet node in IPC forwarder mode, reading config from env.
///
/// The ExEx will forward host chain notifications to the rollup process
/// over a Unix domain socket at the given path.
pub fn node_with_ipc_from_env() -> eyre::Result<()> {
    let socket_path = std::env::var("SIGNET_IPC_SOCKET")
        .unwrap_or_else(|_| "/tmp/signet-rollup.sock".to_string());
    node_with_ipc(socket_path)
}

/// Start the Signet node in IPC forwarder mode.
///
/// Installs a thin ExEx that serializes host chain notifications and sends
/// them over a Unix domain socket to the standalone rollup process.
pub fn node_with_ipc(socket_path: impl Into<std::path::PathBuf> + Send + 'static) -> eyre::Result<()> {
    let socket_path = socket_path.into();

    reth::cli::Cli::parse_args().run(|builder, _| async move {
        let handle = builder
            .node(reth_node_ethereum::EthereumNode::default())
            .install_exex("Signet", move |ctx| async move {
                Ok(forward::ExExForwarder::new(ctx, socket_path).start())
            })
            .launch()
            .await
            .inspect_err(|err| tracing::error!(%err, "Failed to boot"))?;

        handle.wait_for_node_exit().await
    })
}

/// Start the Signet node using the legacy single-process mode.
///
/// This opens both the host chain MDBX and the rollup MDBX in the same
/// process. For production use with heavy sync loads, prefer
/// [`node_with_ipc`] to avoid MDBX corruption.
pub fn node(config: SignetNodeConfig) -> eyre::Result<()> {
    reth::cli::Cli::parse_args().run(|builder, _| async move {
        let prune_config = builder.config().prune_config();

        let handle = builder
            .node(reth_node_ethereum::EthereumNode::default())
            .install_exex("Signet", move |ctx| async {
                let chain_spec: Arc<_> = config.chain_spec().clone();

                // Open the database provider factory.
                let mut factory = ProviderFactory::new_with_database_path(
                    config.database_path(),
                    chain_spec,
                    Default::default(),
                    config.static_file_rw()?,
                    config.open_rocks_db()?,
                    reth::tasks::Runtime::with_existing_handle(tokio::runtime::Handle::current())?,
                )?;

                if let Some(prune_config) = prune_config {
                    factory = factory.with_prune_modes(prune_config.segments);
                }

                Ok(SignetNodeBuilder::new(config)
                    .with_factory(factory.clone())
                    .with_ctx(ctx)
                    .with_client(CLIENT.clone())
                    .build()?
                    .0
                    .start())
            })
            .launch()
            .await
            .inspect_err(|err| tracing::error!(%err, "Failed to boot"))?;

        handle.wait_for_node_exit().await
    })
}
