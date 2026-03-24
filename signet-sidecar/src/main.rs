fn main() -> eyre::Result<()> {
    #[allow(deprecated)]
    let _guard = init4_bin_base::init4();
    signet_sidecar::run()
}
