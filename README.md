# Signet Node

> Binaries for the Signet Node, a rollup node build as a reth
> [ExEx](https://www.paradigm.xyz/2024/05/reth-exex).

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
- Rust (>=1.85.0)

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

`signet-node` can be configured with the following environment variables. This
block contains format examples, not sane defaults.

```ignore
# URL of a blob explorer, required
BLOB_EXPLORER_URL="https://your-blob-explorer"
# Consensus layer URL, optional
SIGNET_CL_URL="https://your-consensus-layer"
# Pylon node URL, optional
SIGNET_PYLON_URL="https://your-pylon-url"
# Path at which to store Signet's static file database, required
SIGNET_STATIC_PATH="/path/to/your/signet/static"
# Filepath at which to store Signet's MDBX database, required
SIGNET_DATABASE_PATH="/path/to/your/signet/db"
# URL to which to forward raw transactions, optional
TX_FORWARD_URL="https://your-tx-cache.url"
# RPC port to serve JSON-RPC requests, optional
RPC_PORT=8546
# Websocket port to serve JSON-RPC requests, optional
WS_RPC_PORT=8547
# IPC endpoint to serve JSON-RPC requests, optional
IPC_ENDPOINT="/tmp/signet.ipc"
# The name of the chain. If set, some other environment variables are ignored.
# optional
CHAIN_NAME="pecorino"
```

The following environment variables are required if `CHAIN_NAME` is not set:

```ignore
# A filepath to the genesis JSON file.
GENESIS_JSON_PATH="/path/to/your/genesis.json"
# The start timestamp of the chain in seconds.
START_TIMESTAMP="1740709768"
# The number of the slot containing the start timestamp.
SLOT_OFFSET="0"
# The slot duration of the chain in seconds.
SLOT_DURATION="12"
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
