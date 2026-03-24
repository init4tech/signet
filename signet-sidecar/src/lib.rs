#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![deny(unused_must_use, rust_2018_idioms)]
#![warn(missing_docs, missing_copy_implementations, missing_debug_implementations)]

//! # Signet Sidecar
//!
//! Standalone signet node binary that follows the host chain via RPC.
//! No reth dependency.

use init4_bin_base::utils::{
    from_env::{FromEnv, FromEnvVar},
    provider::PubSubConfig,
};
use openssl as _;
use signet_host_rpc::{HostRpcConfig, RpcAliasOracle};
use signet_node::SignetNodeBuilder;
use signet_node_config::SignetNodeConfig;
use signet_rpc::{ServeConfigEnv, StorageRpcConfigEnv};
use std::sync::{Arc, LazyLock};
use tokio_util::sync::CancellationToken;
use tracing as _;

/// Global reqwest client.
static CLIENT: LazyLock<reqwest::Client> =
    LazyLock::new(|| reqwest::Client::builder().use_rustls_tls().build().unwrap());

/// Run the sidecar node. This blocks until the node exits.
pub fn run() -> eyre::Result<()> {
    tokio::runtime::Builder::new_multi_thread().enable_all().build()?.block_on(run_inner())
}

async fn run_inner() -> eyre::Result<()> {
    let config = SignetNodeConfig::from_env()?;
    let host_config = HostRpcConfig::from_env()?;
    let serve_config = ServeConfigEnv::from_env()?.try_into()?;
    let rpc_config = StorageRpcConfigEnv::from_env()?.into();

    let cancel = CancellationToken::new();
    let storage = Arc::new(config.storage().build_storage(cancel.clone()).await?);

    let slot_calculator = config.slot_calculator();
    let notifier_builder = host_config.into_builder(slot_calculator).await?;

    // Connect a separate provider for the alias oracle, since
    // `RpcHostNotifierBuilder` does not expose its inner provider.
    let alias_provider = PubSubConfig::from_env_var("SIGNET_HOST_URL")?;
    let alias_oracle = RpcAliasOracle::new(alias_provider.connect().await?);
    let notifier = notifier_builder.build().await?;

    let blob_cacher = signet_blobber::BlobFetcher::builder()
        .with_config(config.block_extractor(), CLIENT.clone())?
        .build_cache()
        .spawn();

    let (node, _status) = SignetNodeBuilder::new(config)
        .with_notifier(notifier)
        .with_storage(storage)
        .with_alias_oracle(alias_oracle)
        .with_blob_cacher(blob_cacher)
        .with_serve_config(serve_config)
        .with_rpc_config(rpc_config)
        .with_client(CLIENT.clone())
        .build()
        .await?;

    node.start().await
}
