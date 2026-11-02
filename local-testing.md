# Local testing

Use a local-only Freenet node for end-to-end testing. It stores the contract
only on that node; this workflow does not publish to the Freenet network.
Do not use `fdev publish` for local testing.

## Fast checks

From the project root:

```sh
cargo test -p zk-notepad-shared --features hash
cargo test -p container
npm run build
```

These check the shared Poseidon2 encoding, contract logic, and frontend build.
They do not generate or verify a real Ziren proof.

## End-to-end test

The native prover needs the Ziren guest toolchain described in the
[Ziren installation guide](https://github.com/ProjectZKM/Ziren/blob/0e2d38388e580f3e061e9bb13d4173d6a614b6a6/docs/src/introduction/installation.md).
The active Rust toolchain also needs the WASM target:

```sh
rustup target add wasm32-unknown-unknown
```

### 1. Start an isolated local-only node

In a terminal, from the project root:

```sh
freenet local --data-dir "$PWD/.freenet-local"
```

This runs the node in local-only mode. Its WebSocket API defaults to
`127.0.0.1:7509`.

### 2. Build the webapp and create a proof candidate

In a second terminal:

```sh
cd /e/proj/zk-example-freenet
unset FREENET_NODE_URL

npm install
PATH="$PWD/node_modules/.bin:$PATH" fdev build

head -c 32 /dev/urandom > secret.bin
cargo run -p notepad-client -- setup \
  --secret secret.bin \
  --output contract-parameters.json \
  --proving-key notepad-proving-key.bin
cargo run -p notepad-client -- prove \
  --secret secret.bin \
  --parameters contract-parameters.json \
  --proving-key notepad-proving-key.bin \
  --counter 1 \
  --text "A local test note" \
  --output candidate.json
cargo run -p notepad-client -- verify \
  --parameters contract-parameters.json \
  --candidate candidate.json
```

`fdev build` packages the contract at `build/freenet/container`. The `PATH`
prefix allows `fdev` to find the project's local webpack executable.
`contract-parameters.json` and `notepad-proving-key.bin` are public artifacts;
keep `secret.bin` private. The `verify` command checks the candidate offline
without needing the secret or proving key.

### 3. Put the contract on the local node

```sh
fdev local execute put \
  --code build/freenet/container \
  --parameters contract-parameters.json \
  contract
```

Copy the contract ID printed by the command. This is a local-node PUT, not a
network publish.

### 4. Submit and inspect the update

Open the webapp through the local node:

```sh
freenet open freenet://<CONTRACT_ID>/
```

Load `candidate.json` in the page and submit it. To test the same update
without the browser, use:

```sh
fdev local execute update <CONTRACT_ID> candidate.json
fdev local execute get <CONTRACT_ID> --output state.json
```

The returned state should contain the winning counter and text. To test
rejection, make a copy of the candidate with a corrupted proof and submit that;
the contract should reject it.

The frontend connects to the WebSocket endpoint on the page's origin. Use the
local node's webapp URL above; `npm start` serves on port 8080 without a proxy
to the node's WebSocket API, so it is not the direct end-to-end UI path.
