# Signet Node

## Versioning

- This repo uses a chain-oriented versioning scheme:
  `<hard fork>.<configuration>.<patch>`
- Each binary in the workspace (`signet`, `signet-exex`, `signet-sidecar`)
  has its own independent version. Configuration changes may affect one binary
  but not others.
- When bumping versions, only bump the binary that is actually affected by
  the change. Do NOT synchronize versions across binaries.

## Dependencies

- **signet**: node-components pinned to legacy git tag (0.16 line). Uses reth.
- **signet-exex**: all node-components deps pinned to the same git tag (0.17+
  line) because `signet-host-reth` is git-only. Mixing git and crates.io
  sources for the same repo causes duplicate crate versions. Uses reth.
- **signet-sidecar**: node-components from crates.io (0.17+ line). No git-only
  deps, no reth dependency.
- When updating node-components for exex, pin ALL its deps to the same git
  tag. Do NOT mix crates.io and git sources for `init4tech/node-components`.

## Tags and Releases

- Use prefixed git tags: `<binary>/v<version>` (e.g., `signet/v1.0.0-rc.9`,
  `signet-exex/v1.0.0-rc.9`).
- Each tag gets its own GitHub release, titled `<binary> v<version>`.
- When only one binary changes, only tag and release that binary.
