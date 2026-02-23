fn main() -> eyre::Result<()> {
    // If SIGNET_IPC_SOCKET is set, run in IPC forwarder mode.
    // Otherwise, run in legacy single-process mode.
    if std::env::var("SIGNET_IPC_SOCKET").is_ok() {
        signet::node_with_ipc_from_env()
    } else {
        signet::node_from_env()
    }
}
