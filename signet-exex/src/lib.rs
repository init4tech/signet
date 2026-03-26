#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![deny(unused_must_use, rust_2018_idioms)]
#![warn(missing_docs, missing_copy_implementations, missing_debug_implementations)]

//! # Signet ExEx
//!
//! Signet node binary running as a reth Execution Extension (ExEx).

use init4_bin_base::utils::from_env::FromEnv;
use openssl as _;
use signet_blobber::BlobFetcher;
use signet_host_reth::{RethAliasOracleFactory, RethBlobSource, decompose_exex_context};
use signet_node::SignetNodeBuilder;
use signet_node_config::SignetNodeConfig;
use std::sync::{Arc, LazyLock};
use tokio_util::sync::CancellationToken;

/// Global reqwest client.
static CLIENT: LazyLock<reqwest::Client> =
    LazyLock::new(|| reqwest::Client::builder().use_rustls_tls().build().unwrap());

/// Start the Signet ExEx node, reading config from env.
///
/// When `SIGNET_DISABLE_EXEX=true` (or `1`), the node runs as vanilla reth
/// without the Signet ExEx. This is used during initial chain sync so that
/// reth can sync to tip before the ExEx is enabled.
pub fn node_from_env() -> eyre::Result<()> {
    if std::env::var("SIGNET_DISABLE_EXEX").is_ok_and(|v| v == "true" || v == "1") {
        return node_without_exex();
    }
    SignetNodeConfig::from_env().map(node)?
}

/// Run the node as vanilla reth without the Signet ExEx.
fn node_without_exex() -> eyre::Result<()> {
    reth::cli::Cli::parse_args().run(|builder, _| async move {
        let handle = builder
            .node(reth_node_ethereum::EthereumNode::default())
            .launch()
            .await
            .inspect_err(|err| tracing::error!(%err, "Failed to boot vanilla reth"))?;

        handle.wait_for_node_exit().await
    })
}

/// Start the Signet ExEx node with the provided config.
pub(crate) fn node(config: SignetNodeConfig) -> eyre::Result<()> {
    reth::cli::Cli::parse_args().run(|builder, _| async move {
        let handle = builder
            .node(reth_node_ethereum::EthereumNode::default())
            .install_exex("Signet", move |ctx| async move {
                let provider = ctx.provider().clone();
                let decomposed = decompose_exex_context(ctx);

                let cancel = CancellationToken::new();
                spawn_shutdown_handler(cancel.clone());
                let storage = Arc::new(config.storage().build_storage(cancel.clone()).await?);

                let alias_oracle = RethAliasOracleFactory::new(Box::new(provider));

                let blob_cacher = BlobFetcher::builder()
                    .with_source(RethBlobSource(decomposed.pool))
                    .with_config(config.block_extractor(), CLIENT.clone())?
                    .build_cache()
                    .spawn();

                Ok(SignetNodeBuilder::new(config)
                    .with_notifier(decomposed.notifier)
                    .with_storage(storage)
                    .with_alias_oracle(alias_oracle)
                    .with_blob_cacher(blob_cacher)
                    .with_serve_config(decomposed.serve_config)
                    .with_rpc_config(decomposed.rpc_config)
                    .with_client(CLIENT.clone())
                    .build()
                    .await?
                    .0
                    .start())
            })
            .launch()
            .await
            .inspect_err(|err| tracing::error!(%err, "Failed to boot"))?;

        handle.wait_for_node_exit().await
    })
}

/// Spawn a task that cancels the given token on receipt of a shutdown
/// signal (SIGINT on all platforms, plus SIGTERM on unix).
fn spawn_shutdown_handler(cancel: CancellationToken) {
    tokio::spawn(async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{SignalKind, signal};
            let mut sigterm =
                signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = sigterm.recv() => {}
            }
        }
        #[cfg(not(unix))]
        tokio::signal::ctrl_c().await.ok();

        cancel.cancel();
    });
}
