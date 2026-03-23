# Signet Node

[![CI](https://github.com/init4tech/signet-node/actions/workflows/rust.yml/badge.svg)](https://github.com/init4tech/signet-node/actions/workflows/rust.yml)
[![Docker](https://github.com/init4tech/signet-node/actions/workflows/docker-ecr.yml/badge.svg)](https://github.com/init4tech/signet-node/actions/workflows/docker-ecr.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)

Binaries for the Signet Node, a rollup node built as a reth [ExEx](https://www.paradigm.xyz/2024/05/reth-exex).

## Versioning

Signet-node does not follow [semantic versioning](https://semver.org/).
Instead, it uses a chain-oriented versioning scheme:

`<hard fork>.<configuration>.<patch>`

- **Hard fork** version is incremented when a version supports a new hard fork (even if the hard fork is not yet active).
- **Configuration** version is incremented when backwards-incompatible changes are made to the node configuration environment variables.
- **Patch** version is incremented on all other changes, including bug fixes and new features.

## Prerequisites

- Docker
- Make
- Rust (≥1.85.0)

## Quick Start

### Docker

Signet Node is delivered as a Docker container.

```sh
make docker  # Builds image tagged as signet:latest
```

### Building from Source

```sh
make build
```

Then run the binary:

```sh
signet node \
    --chain pecorino \
    --authrpc.jwtsecret /path/to/your/jwt.secret \
    --http \
    --http.addr 127.0.0.1 \
    --http.api eth,net,trace,txpool,web3,rpc,debug,ots
```

## Configuration

### Environment Variables

`signet-node` can be configured with the following environment variables:

| Variable | Required | Description |
|----------|----------|-------------|
| `BLOB_EXPLORER_URL` | Yes | URL of a blob explorer |
| `SIGNET_CL_URL` | No | Consensus layer URL |
| `SIGNET_PYLON_URL` | No | Pylon node URL |
| `SIGNET_STATIC_PATH` | Yes | Path to store Signet's static file database |
| `SIGNET_DATABASE_PATH` | Yes | Path to store Signet's MDBX database |
| `TX_FORWARD_URL` | No | URL to forward raw transactions |
| `RPC_PORT` | No | RPC port for JSON-RPC requests (default: 8546) |
| `WS_RPC_PORT` | No | WebSocket port for JSON-RPC requests (default: 8547) |
| `IPC_ENDPOINT` | No | IPC endpoint for JSON-RPC requests |
| `CHAIN_NAME` | No | Chain name (e.g., "pecorino"). If set, some other variables are ignored. |

If `CHAIN_NAME` is not set, these additional variables are required:

| Variable | Description |
|----------|-------------|
| `GENESIS_JSON_PATH` | Path to the genesis JSON file |
| `START_TIMESTAMP` | Start timestamp of the chain in seconds |
| `SLOT_OFFSET` | Slot number containing the start timestamp |
| `SLOT_DURATION` | Slot duration in seconds |

### Docker Compose

1. Generate a JWT token:

   ```sh
   mkdir -p jwttoken && echo 'abcdabcdabcdabcdabcdabcdabcdabcdabcdabcdabcdabcdabcdabcdabcdabcd' > jwttoken/jwt.hex
   chmod +x jwttoken/jwt.hex
   ```

2. Start the node:

   ```sh
   docker-compose up
   ```

3. Verify the node is running:

   ```sh
   cast rpc --rpc-url http://localhost:8545/ eth_getBlockByNumber "latest" "false"
   ```

> **Note:** If your Lighthouse client throws timeout errors, try using an alternative checkpoint sync URL.

## Makefile Commands

| Command | Description |
|---------|-------------|
| `make build` | Compile in release mode |
| `make clean` | Remove generated files |
| `make check` | Quick error check without producing executable |
| `make test` | Run tests |
| `make fmt` | Format code |
| `make lint` | Lint code with clippy |
| `make docker` | Build Docker image (`signet:latest`) |
| `make pecorino` | Build Pecorino Docker image (`pecorino-signet:latest`) |
| `make dev` | Start docker-compose with lighthouse and signet node |
| `make run` | Run the signet binary |

## Testing

```sh
make test
```

## License

This project is licensed under the [MIT License](LICENSE).
