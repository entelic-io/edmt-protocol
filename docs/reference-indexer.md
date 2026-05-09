# Rust Reference Scanner

The Rust implementation in `implementations/rust/edmt-indexer-lite` is a
minimal runnable calldata scanner and parser.

It exists to make the calldata envelope concrete:

- fetch a block range through Ethereum JSON-RPC;
- scan transaction `input` fields;
- recognize the exact `data:,` prefix;
- parse the payload as a single JSON object;
- classify known eDMT operation envelopes;
- write JSONL records for inspection.

This implementation is non-normative. It is not a full compliant indexer and
does not maintain canonical balances, finality, reorg handling, capture fee
accounting, or application state.

The authoritative protocol documents remain:

- `docs/protocol.md`
- `docs/indexer.md`
- `docs/calldata.md`

If this reference scanner conflicts with the specification documents, the
specification documents win.

## Running

```sh
cd implementations/rust/edmt-indexer-lite
cargo run -- \
  --rpc-url "https://example.invalid/rpc" \
  --from-block 12965000 \
  --to-block 12965010
```

The RPC URL may also be supplied with `EDMT_RPC_URL`.

## Output Model

The scanner writes JSONL records. Each record is classified as:

- `parsed`: a known eDMT operation envelope was parsed;
- `rejected`: an eDMT envelope was present but malformed;
- `ignored`: the transaction is outside this scanner's envelope scope.

`parsed` is not a state-validity claim.
