//! Remote alias oracle that checks `eth_getCode` via the host chain's
//! HTTP RPC endpoint, with an LRU cache to reduce redundant queries.
//!
//! This is used by the standalone rollup process which does not have
//! direct access to the host chain's MDBX database.

use alloy::{
    consensus::constants::KECCAK_EMPTY,
    primitives::{Address, Bytes, keccak256},
};
use eyre::Context;
use signet_block_processor::{AliasOracle, AliasOracleFactory};
use std::sync::{Arc, Mutex};

/// An LRU cache mapping address → should_alias result.
type AliasCache = lru::LruCache<Address, bool>;

/// Factory that produces [`RemoteAliasOracle`] instances, each sharing the
/// same HTTP RPC URL and LRU cache.
#[derive(Debug, Clone)]
pub struct RemoteAliasOracleFactory {
    /// The host chain's HTTP RPC URL (e.g. http://localhost:8545).
    rpc_url: reqwest::Url,
    /// Shared HTTP client.
    client: reqwest::Client,
    /// Shared LRU cache for alias results.
    cache: Arc<Mutex<AliasCache>>,
}

impl RemoteAliasOracleFactory {
    /// Create a new factory.
    ///
    /// `cache_size` controls the number of address → alias entries to cache.
    pub fn new(rpc_url: reqwest::Url, client: reqwest::Client, cache_size: usize) -> Self {
        let cache = Arc::new(Mutex::new(AliasCache::new(
            std::num::NonZeroUsize::new(cache_size).expect("cache_size must be > 0"),
        )));
        Self { rpc_url, client, cache }
    }
}

impl AliasOracleFactory for RemoteAliasOracleFactory {
    type Oracle = RemoteAliasOracle;

    fn create(&self) -> eyre::Result<Self::Oracle> {
        Ok(RemoteAliasOracle {
            rpc_url: self.rpc_url.clone(),
            client: self.client.clone(),
            cache: self.cache.clone(),
        })
    }
}

/// An alias oracle that queries the host chain via JSON-RPC `eth_getCode`
/// to determine whether an address should be aliased.
///
/// Results are cached in a shared LRU cache to avoid redundant RPC calls.
#[derive(Debug, Clone)]
pub struct RemoteAliasOracle {
    rpc_url: reqwest::Url,
    client: reqwest::Client,
    cache: Arc<Mutex<AliasCache>>,
}

impl AliasOracle for RemoteAliasOracle {
    fn should_alias(&self, address: Address) -> eyre::Result<bool> {
        // Check cache first
        {
            let mut cache = self
                .cache
                .lock()
                .map_err(|_| eyre::eyre!("alias cache lock poisoned"))?;
            if let Some(&result) = cache.get(&address) {
                return Ok(result);
            }
        }

        // Cache miss — query the host chain's RPC.
        // NB: The AliasOracle trait is sync, so we use block_in_place to
        // call async code. This is acceptable because alias checks are
        // infrequent (once per unique address per sync session) and the
        // LRU cache absorbs repeated lookups.
        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.fetch_should_alias(address))
        })?;

        // Cache the result
        {
            let mut cache = self
                .cache
                .lock()
                .map_err(|_| eyre::eyre!("alias cache lock poisoned"))?;
            cache.put(address, result);
        }

        Ok(result)
    }
}

impl RemoteAliasOracle {
    /// Fetch code for an address via `eth_getCode` at `latest` and determine
    /// whether it should be aliased.
    ///
    /// The logic mirrors the `StateProviderBox` implementation in
    /// `signet-block-processor`:
    /// - No code → not aliased
    /// - EIP-7702 delegation code → not aliased
    /// - Any other code → aliased
    async fn fetch_should_alias(&self, address: Address) -> eyre::Result<bool> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getCode",
            "params": [format!("{address:?}"), "latest"],
            "id": 1
        });

        let resp = self
            .client
            .post(self.rpc_url.clone())
            .json(&body)
            .send()
            .await
            .wrap_err("eth_getCode request failed")?;

        let json: serde_json::Value = resp
            .json()
            .await
            .wrap_err("failed to parse eth_getCode response")?;

        let code_hex = json["result"]
            .as_str()
            .ok_or_else(|| eyre::eyre!("eth_getCode returned no result"))?;

        let code: Bytes = code_hex
            .parse()
            .wrap_err("failed to parse code hex")?;

        // No code at this address — not a contract, don't alias.
        if code.is_empty() {
            return Ok(false);
        }

        // Check if the code hash is KECCAK_EMPTY (no real code).
        let code_hash = keccak256(&code);
        if code_hash == KECCAK_EMPTY {
            return Ok(false);
        }

        // Check for EIP-7702 delegation: starts with 0xef0100
        if code.starts_with(&[0xef, 0x01, 0x00]) {
            return Ok(false);
        }

        // Has real non-delegation code — alias it.
        Ok(true)
    }
}
