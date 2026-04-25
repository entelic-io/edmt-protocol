# eDMT Indexer Specification

This document defines the behavior expected from a compliant eDMT indexer.

The goal is deterministic convergence: independent implementations should derive the same confirmed protocol state from the same finalized Ethereum history.

If this document conflicts with [Protocol Specification](protocol.md), the protocol specification wins.

## 1. Requirements Language

The keywords MUST, MUST NOT, SHOULD, SHOULD NOT, and MAY are used in the RFC 2119 sense.

## 2. Inputs

A compliant indexer reads:

| Data | Ethereum RPC method | Purpose |
| --- | --- | --- |
| Block header | `eth_getBlockByNumber`, full transactions disabled | compute `burn(N)` and finality |
| Full block | `eth_getBlockByNumber`, full transactions enabled | scan transaction order and calldata |
| Transaction | block transaction list | read `from`, `to`, `input`, `transactionIndex`, status references |
| Receipt | `eth_getTransactionReceipt` | determine whether a transaction reverted |

The indexer MUST compute `burn(N)` itself:

```text
burn(N) = floor(baseFeePerGas(N) * gasUsed(N) / 10^9)
```

The computation MUST use integer arithmetic. Floating point arithmetic MUST NOT be used.

## 3. Chain Context Sanity Checks

Before indexing, an implementation MUST verify:

1. `chain_id` is explicitly configured;
2. `eip1559_activation_block` is explicitly configured;
3. `eth_chainId` equals configured `chain_id`;
4. the block at `eip1559_activation_block` has non-null `baseFeePerGas`;
5. if `eip1559_activation_block > 0`, the previous block has null `baseFeePerGas`.

If any check fails, the indexer MUST refuse to start. It MUST NOT fall back to mainnet values or any other default.

## 4. Block Processing Order

Blocks MUST be processed in canonical chain order.

Within a block, transactions MUST be processed by ascending `transactionIndex`.

Only transactions in successfully executed Ethereum transactions enter protocol consideration. Reverted transactions MUST be ignored.

## 5. Calldata Parsing

For each transaction:

1. read `input`;
2. decode bytes as UTF-8;
3. require exact prefix `data:,`;
4. parse the remainder as one JSON object.

If decoding fails, the prefix is absent, or the JSON payload is absent, the transaction is not an eDMT protocol transaction and MUST be ignored.

If JSON parsing fails after the `data:,` prefix is present, the operation is rejected as malformed protocol calldata.

## 6. Ignore vs Reject

The distinction is important.

**Ignore** means the transaction is outside protocol state:

- no state change;
- no protocol failure;
- MAY be absent from audit logs.

**Reject** means the transaction was recognized as protocol calldata but did not satisfy the rules:

- no state change;
- SHOULD be represented in audit logs as a rejected protocol operation.

Ignored cases:

- calldata does not begin with `data:,`;
- decoded JSON has no `"p"` field;
- `"p" != "edmt"`;
- `"op"` is not a known protocol operation.

Rejected cases:

- malformed JSON after `data:,`;
- required field missing;
- required field has wrong type;
- field value violates protocol rules;
- insufficient balance;
- duplicate mint after first-is-first has already resolved;
- any failed item inside `emt-batch-transfer`.

## 7. State Model

An indexer may choose any physical database schema, but it MUST represent the following logical state.

### 7.1 Tickers

Each deployed ticker:

- `tick`
- `elem_tx_hash`
- `deployer`
- `deploy_block`
- `deploy_tx`
- `deploy_tx_index`
- `dt`

For canonical eNAT, `tick` is `"enat"`.

### 7.2 Mint Records

For each `(tick, blk)`:

- `owner`
- `burn_amount`
- `mint_tx`
- `mint_block`
- `mint_tx_index`
- confirmation status

### 7.3 Whole Holdings

A whole holding represents ownership of one unfragmented minted block.

Fields:

- `blk`
- `burn_amount`
- `mint_tx`
- `mint_block`
- `acquired_at_tx`
- `acquired_at_block`
- `acquired_at_tx_index`

Whole holdings MUST be ordered by:

1. `acquired_at_block`;
2. `acquired_at_tx_index`;
3. deterministic tie-breaker if needed.

### 7.4 Fragment Balance

Fragments are fungible gwei amounts held in a FIFO queue.

Each fragment entry has:

- `amount`
- `received_at_tx`
- `received_at_block`
- `received_at_tx_index`

Fragment entries MUST NOT be merged merely because they are adjacent or have equal values. Every `add_fragment` event appends a distinct FIFO entry.

### 7.5 Burn Log

Burn events SHOULD be represented as:

- `tick`
- `tx_hash`
- `from`
- `src`
- `amount`
- `block`
- `tx_index`

The indexer MUST maintain `total_burned` per ticker.

## 8. Mint Processing

For `emt-mint`:

1. validate schema;
2. verify ticker exists;
3. parse `blk` as a decimal positive integer string;
4. require `blk >= eip1559_activation_block`;
5. require mint transaction block number `>= blk`;
6. compute `burn(blk)`;
7. require `burn(blk) >= 1`;
8. require transaction `to == from`;
9. require `(tick, blk)` not already claimed by an earlier valid mint;
10. assign ownership to `tx.from`.

If multiple valid mint candidates for the same `(tick, blk)` appear in the same block, the lowest `transactionIndex` wins.

## 9. Transfer Processing

Before applying `emt-transfer`, validate:

- `amt` is a decimal positive integer string;
- `to` is an Ethereum address;
- `src` is either a decimal block number string or `"balance"`;
- `to` is not equal to `tx.from` after address normalization.

### 9.1 `src = "<block_number>"`

Algorithm:

```text
whole = find whole holding owned by tx.from for block_number
if not found: reject
if amt > whole.burn_amount: reject
if amt == whole.burn_amount:
    remove whole from sender
    add whole to recipient with acquired_at = current tx
else:
    remove whole from sender
    append fragment(whole.burn_amount - amt) to sender FIFO
    append fragment(amt) to recipient FIFO
```

Partial split MUST NOT recreate a smaller whole holding for the sender. The remainder becomes fragment balance permanently.

### 9.2 `src = "balance"`

Algorithm:

```text
if sender fragment total < amt: reject
consume sender fragments FIFO oldest-to-newest until amt is paid
append one fragment(amt) to recipient FIFO
```

The indexer MUST NOT split whole holdings when `src = "balance"`.

## 10. Batch Transfer Processing

For `emt-batch-transfer`:

1. validate `items` is a non-empty array;
2. require `items.length <= 256`;
3. validate every item before mutation when possible;
4. execute items in array order against a temporary state snapshot;
5. if any item fails, restore the snapshot and reject the whole batch;
6. if every item succeeds, commit all changes.

Earlier successful items in the same batch are visible to later items. This is part of the semantics.

## 11. Burn Processing

For `emt-burn`:

- validate `amt`;
- validate `src`;
- ignore `to` if present.

### 11.1 `src = "<block_number>"`

```text
whole = find whole holding owned by tx.from for block_number
if not found: reject
if amt > whole.burn_amount: reject
if amt == whole.burn_amount:
    remove whole
    total_burned += amt
else:
    remove whole
    append fragment(whole.burn_amount - amt) to sender FIFO
    total_burned += amt
record burn log
```

### 11.2 `src = "balance"`

```text
if sender fragment total < amt: reject
consume sender fragments FIFO oldest-to-newest until amt is paid
total_burned += amt
record burn log
```

No recipient receives burned amount.

## 12. Finality and Reorg Handling

An indexer SHOULD model two states:

| State | Meaning |
| --- | --- |
| pending | observed on chain but not finalized |
| confirmed | past the configured finality threshold |

Before finality, pending state may be rolled back and replayed when the canonical chain changes.

After finality, confirmed state MUST NOT be automatically rolled back.

If a reorg exceeds the finality threshold, the indexer SHOULD halt or alert rather than silently rewrite confirmed state.

## 13. State Root

A compliant indexer SHOULD be able to publish a deterministic state root for a processed block.

A recommended structure:

```text
state_root(N) = keccak256(
  ticker_root(N) ||
  mint_root(N) ||
  holdings_root(N) ||
  burned_root(N)
)
```

Where each root is computed from sorted deterministic serializations:

- tickers sorted by `tick`;
- mint records sorted by `(tick, blk)`;
- holdings sorted by `(tick, owner)`;
- whole holdings sorted by acquisition order;
- fragment FIFO entries serialized in FIFO order;
- burn logs sorted by `(block, tx_index, tx_hash)`.

All integers SHOULD be serialized as unsigned 32-byte big-endian values. Addresses SHOULD be lowercased 20-byte values. Strings SHOULD be UTF-8.

Implementations that choose a different state root convention SHOULD publish it explicitly.

## 14. API Surface

A public indexer SHOULD expose JSON query APIs sufficient to verify state. Endpoint names MAY differ, but equivalent functionality SHOULD exist:

- list deployed tickers;
- query ticker metadata;
- query block mint status;
- query holder list;
- query address holdings, including whole and fragment state;
- query burn log and total burned;
- query sync status;
- query state root at a given block.

API design is not part of protocol consensus. Indexer state derivation is.

## 15. Conformance Checklist

A compliant indexer MUST:

- verify chain context before starting;
- process blocks and transactions in canonical order;
- compute burn from block headers with integer arithmetic;
- implement exact `data:,` JSON parsing rules;
- distinguish ignore from reject;
- implement first-is-first mint resolution;
- implement whole and fragment holdings separately;
- preserve fragment FIFO entries;
- implement `src = "<block_number>"` and `src = "balance"` exactly;
- implement atomic batch transfer;
- implement protocol burn distinct from zero-address transfer;
- handle pre-finality reorgs without changing confirmed finality semantics;
- ignore unknown fields and unknown operations as specified.

## 16. Outside This Specification

This document does not specify:

- implementation language;
- database schema;
- hosting setup;
- monitoring system;
- frontend design;
- application-layer logic.
