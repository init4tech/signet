// TODO: ENG-480 - Remove allow dead code macros
// https://linear.app/initiates/issue/ENG-480/remove-allow-dead-code-proc-macros
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![deny(unused_must_use, rust_2018_idioms)]
#![warn(missing_docs, missing_copy_implementations, missing_debug_implementations)]

//! # Signet

use openssl as _; // silences clippy warning

use init4_bin_base::utils::from_env::FromEnv;
use reth::providers::{ProviderFactory, StateProviderFactory, providers::BlockchainProvider};
use signet_node::SignetNode;
use signet_node_config::SignetNodeConfig;
use std::sync::{Arc, LazyLock};

/// The global reqwest client used throughout the node.
pub static CLIENT: LazyLock<reqwest::Client> =
    LazyLock::new(|| reqwest::Client::builder().use_rustls_tls().build().unwrap());

/// Start the Signet node, reading config from env.
pub fn node_from_env() -> eyre::Result<()> {
    SignetNodeConfig::from_env().map(node)?
}

/// State the Signet node, using the provided config.
pub fn node(config: SignetNodeConfig) -> eyre::Result<()> {
    reth::cli::Cli::parse_args().run(|builder, _| async move {
        let db_args = reth_db::mdbx::DatabaseArguments::default();

        let prune_config = builder.config().prune_config();

        let handle = builder
            .node(reth_node_ethereum::EthereumNode::default())
            .install_exex("Signet", move |ctx| async {
                let chain_spec: Arc<_> = config.chain_spec().clone();

                // Open the database provider factory.
                let mut factory = ProviderFactory::new_with_database_path(
                    config.database_path(),
                    chain_spec,
                    db_args,
                    config.static_file_rw()?,
                )?;
                if let Some(prune_config) = prune_config {
                    factory = factory.with_prune_modes(prune_config.segments);
                }

                // This allows the node to look up contract status.
                let boxed_factory: Box<dyn StateProviderFactory> =
                    Box::new(BlockchainProvider::new(factory.clone())?);

                Ok(SignetNode::new(ctx, config, factory.clone(), boxed_factory, CLIENT.clone())?
                    .0
                    .start())
            })
            .launch()
            .await
            .inspect_err(|err| tracing::error!(%err, "Failed to boot"))?;

        handle.wait_for_node_exit().await
    })
}
