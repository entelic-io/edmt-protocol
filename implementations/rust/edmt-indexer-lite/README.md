# eDMT Indexer Lite

Minimal runnable eDMT calldata scanner and parser.

This crate reads Ethereum blocks through JSON-RPC, scans transaction calldata,
parses eDMT `data:,` envelopes, and writes JSONL records to standard output.

It is intentionally small. It does not maintain balances, finality, reorg
handling, capture fee accounting, or application state.

## Scope

This tool is useful for:

- inspecting eDMT calldata in a block range;
- checking whether a transaction contains a known eDMT operation envelope;
- generating simple JSONL datasets for downstream experiments;
- learning how the calldata envelope in `docs/calldata.md` is recognized.

This tool is not:

- a full compliant indexer;
- a canonical balance engine;
- a settlement or application state component;
- a replacement for `docs/protocol.md` or `docs/indexer.md`.

If this implementation conflicts with the specification documents, the
specification documents are authoritative.

## Usage

```sh
cargo run -- \
  --rpc-url "https://example.invalid/rpc" \
  --from-block 12965000 \
  --to-block 12965010
```

The RPC URL may also be supplied through `EDMT_RPC_URL`:

```sh
EDMT_RPC_URL="https://example.invalid/rpc" cargo run -- \
  --from-block 12965000 \
  --to-block 12965010
```

By default, non-eDMT transactions are omitted. Include them with:

```sh
cargo run -- \
  --rpc-url "https://example.invalid/rpc" \
  --from-block 12965000 \
  --to-block 12965000 \
  --include-ignored
```

## Output

Each output line is one JSON object:

```json
{"block_number":12965000,"tx_hash":"0x...","classification":"parsed","operation":"emt-mint","payload":{"p":"edmt","op":"emt-mint","tick":"enat","blk":"12965000"}}
```

Classifications:

- `parsed`: the transaction contains a known eDMT operation envelope.
- `rejected`: the transaction uses the eDMT envelope but the JSON envelope is malformed.
- `ignored`: the transaction is outside this tool's eDMT envelope scope.

`parsed` does not mean state-valid. State validity requires the full protocol
rules, canonical ordering, finality, and state transitions described in the
specification.

## Tests

```sh
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

The test suite does not require an RPC endpoint.
Unit tests currently live inline in `src/lib.rs`.
