# Signet Node

> Code and tooling for developing the Signet Node in an [ExEx](https://www.paradigm.xyz/2024/05/reth-exex).

![rust](https://github.com/init4tech/signet-node/actions/workflows/rust.yml/badge.svg) ![ecr](https://github.com/init4tech/signet-node/actions/workflows/docker-ecr.yml/badge.svg)

## Versioning

Signet-node does not follow [semantic versioning](https://semver.org/).
Instead, it uses a chain-oriented versioning scheme:

`<hard fork>.<configuration>.<patch>`

The hard fork version is incremented when a version supports a new hard fork
(even if the hard fork is not yet active). The configuration version is
incremented when backwards-incompatible changes are made to the node
configuration environment variables. I.e. when simply upgrading and rebooting
the node would cause unexpected behavior. The patch version is incremented on
all other changes, including bug fixes and new features.

## Development

> Instructions and notes for running a node locally for development and testing purposes.

### Prerequisites

- Docker
- Make
- Rust (1.85.0)

### Docker

Signet Node is delivered as a Docker container.

`make docker` builds a Docker image tagged as `signet:latest`.

### Binaries

`make build` will compile a production-ready version of Signet Node.

You can then run the binary with the following:

```sh
signet node \
    --chain pecorino \
    --authrpc.jwtsecret /path/to/your/jwt.secret \
    --http \
    --http.addr 127.0.0.1 \
    --http.api eth,net,trace,txpool,web3,rpc,debug,ots
```

### Env Configuration

`signet-node` can be configured with the following environment variables. Sane defaults are provided.

```sh
# Path to the static files.
export SIGNET_STATIC_PATH="$HOME/.local/share/exex/holesky/signet/static"
# Path to the database.
export SIGNET_DATABASE_PATH="$HOME/.local/share/exex/holesky/signet/db"
# URL to which to forward raw transactions. Of the format "https://..."
export TX_FORWARD_URL="https://your-tx-forwarder.url"
# Port for serving JSON-RPC over HTTP
export RPC_PORT="8546"
# Port for serving JSON-RPC over WS
export WS_RPC_PORT="8547"
# Endpoint to serve JSON-RPC over IPC
export IPC_ENDPOINT="/tmp/signet.ipc"
# Genesis file path to parse on startup
export GENESIS_JSON_PATH="./crates/node/genesis.json"
# URL to the consensus layer signet should have access to for fetching blobs
export SIGNET_CL_URL = "https://your-consensus-layer"
# URL to the Pylon node URL signet should have access to for fetching blobs
export SIGNET_PYLON_URL = "https://your-pylon-url"
# Path to your blob explorer URL, if any.
export BLOB_EXPLORER_URL="https://holesky.blobscan.com/"
# Starting timestamp of the Host chain
export START_TIMESTAMP = "1740709768"
# Slot offset of the Host chain
export SLOT_OFFSET = "0"
# Slot duration of the Host chain
export SLOT_DURATION = "12"
```

### Docker-Compose

To run the node, first generate a JWT token for the node to use.

```sh
mkdir -p jwttoken && echo 'abcdabcdabcdabcdabcdabcdabcdabcdabcdabcdabcdabcdabcdabcdabcdabcd' > jwttoken/jwt.hex
```

> Note: You may need to run `chmod +x jwttoken/jwt.hex` to make the file readable.

Once you have created a JWT token, you can start the node using `docker-compose up`.

Then ping `signet-node` via RPC to get the latest rollup block number.

```sh
cast rpc --rpc-url http://localhost:8545/ eth_getBlockByNumber "latest" "false"
```

> Note: If your Lighthouse client throws timeout errors, try using an alternative checkpoint sync URL.

### Makefile

- `build`: Compiles the crate in release mode.
- `clean`: Removes all generated files and directories.
- `check`: Quickly checks your code for errors without producing an executable.
- `test`: Runs tests specified in your Rust crate.
- `fmt`: Formats your Rust code using cargo fmt.
- `lint`: Lints your code to catch common mistakes and improve the code using cargo clippy.
- `docker`: Builds a Docker image tagged `signet:latest`.
- `pecorino`: Builds a Pecorino Docker image tagged `pecorino-signet:latest`
- `dev`: Starts a docker-compose that runs a lighthouse and a signet node.
- `run`: Runs the signet binary

## Testing

To run tests, run the following command

`make test`
