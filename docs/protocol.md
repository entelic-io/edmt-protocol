# eDMT Protocol Specification

This document defines the base eDMT protocol. If another document conflicts with this one, this document is authoritative for the protocol layer.

The protocol layer is calldata-only. It defines how Ethereum transactions are interpreted by compliant indexers. It does not deploy or depend on an asset contract.

## 1. Scope

eDMT derives asset state from two public inputs:

1. finalized Ethereum chain history;
2. calldata payloads that match this specification.

`enat` is the canonical first ticker of the protocol.

The protocol may be instantiated on any EIP-1559 chain by declaring a chain context, but canonical eNAT state is the state derived from Ethereum mainnet.

## 2. Chain Context

Each protocol instance has an explicit chain context:

| Field | Type | Meaning |
| --- | --- | --- |
| `chain_id` | uint64 | EVM chain id returned by `eth_chainId`. |
| `eip1559_activation_block` | uint64 | First block on that chain whose header contains a non-null `baseFeePerGas`. |
| `capture_fee_activation_block` | optional uint64 | Target block threshold at which mint capture fee becomes mandatory. If absent, capture fee is disabled. |

A compliant indexer MUST NOT assume default values for `chain_id` or `eip1559_activation_block`. Both fields MUST be explicitly configured. `capture_fee_activation_block` MAY be absent only when capture fee is disabled for that protocol instance.

For Ethereum mainnet:

| Chain | `chain_id` | `eip1559_activation_block` |
| --- | ---: | ---: |
| Ethereum mainnet | `1` | `12965000` |

The protocol is defined only for blocks:

```text
N >= ChainContext.eip1559_activation_block
```

Blocks before EIP-1559 activation are outside the protocol because they do not contain the base fee field.

### 2.1 Capture Fee Parameters

When `capture_fee_activation_block` is configured, it is evaluated against the target block in `emt-mint.blk`, not against the block that contains the mint transaction.

Default capture fee parameters:

| Parameter | Value | Meaning |
| --- | ---: | --- |
| `window_blocks` | `1000` | Number of previous indexed blocks used for fee statistics. |
| `target_mint_rate_bps` | `3000` | Target post-activation mint rate, in basis points. |
| `min_multiplier_bps` | `2500` | Minimum congestion multiplier, 0.25x. |
| `max_multiplier_bps` | `40000` | Maximum congestion multiplier, 4x. |

For a target block `N`:

- if `capture_fee_activation_block` is absent, capture fee is not required;
- if `N < capture_fee_activation_block`, capture fee is not required;
- if `N >= capture_fee_activation_block`, capture fee is required.

Capture fee is paid and destroyed from the sender's protocol-layer raw fragment balance. It is not paid through an application contract, token allowance, or any other external asset representation.

## 3. Element

The base element is `burn`.

For block `N`:

```text
burn(N) = floor(baseFeePerGas(N) * gasUsed(N) / 10^9)
```

Unit: gwei.

The value MUST be computed from Ethereum block header fields. It MUST NOT be supplied by a user, a transaction payload, or an application.

## 4. Element Registration

The burn element is registered by an Ethereum transaction carrying:

```text
data:,burn.7.element
```

Meaning:

- element name: `burn`;
- field index: `7`, the block header position used by the original element convention for `baseFeePerGas`;
- value formula: the `burn(N)` formula above.

The transaction hash of this registration is referenced by the deploy operation through the `elem` field.

## 5. Calldata Envelope

All protocol operations are ordinary Ethereum transactions whose calldata is the UTF-8 string:

```text
data:,<json>
```

The prefix is exactly six ASCII bytes:

```text
data:,
```

The payload after the prefix MUST be a single JSON object.

The protocol defines five operations:

- `emt-deploy`
- `emt-mint`
- `emt-transfer`
- `emt-batch-transfer`
- `emt-burn`

Unknown operations are ignored and do not affect protocol state.

## 6. Deploy

Deploy registers a ticker.

```json
{
  "p": "edmt",
  "op": "emt-deploy",
  "elem": "<element_tx_hash>",
  "tick": "enat",
  "dt": "n"
}
```

Rules:

- `p` MUST equal `"edmt"`.
- `op` MUST equal `"emt-deploy"`.
- `elem` MUST reference the burn element registration transaction.
- `tick` MUST equal `"enat"` for the canonical first ticker.
- `dt` MUST equal `"n"`.
- The first valid deploy for a ticker wins.
- Later deploys for the same ticker are rejected.

## 7. Mint

Mint claims one eNAT for one block.

```json
{
  "p": "edmt",
  "op": "emt-mint",
  "tick": "enat",
  "blk": "<block_number>",
  "fee": "<optional_capture_fee_gwei>"
}
```

Fields:

| Field | Rule |
| --- | --- |
| `blk` | Decimal positive integer string, target block to claim. |
| `fee` | Optional decimal non-negative integer string, unit gwei. Required only when capture fee applies. |

A block `N` can be minted if and only if all of the following are true:

1. `N >= ChainContext.eip1559_activation_block`;
2. `burn(N) >= 1`;
3. no earlier valid mint has claimed `N` for the same ticker;
4. the mint calldata is valid;
5. the mint transaction is not in a reverted transaction;
6. the Ethereum transaction `to` field equals the transaction `from` field;
7. if capture fee applies, `fee` is present and greater than or equal to the required capture fee;
8. if capture fee applies, the sender has enough raw fragment balance to pay `fee`.

When capture fee applies, `fee` is consumed from the sender's fragment FIFO queue and destroyed, reducing protocol circulation by the consumed amount. If `fee` is greater than the required capture fee, the full declared amount is consumed and destroyed. Overpayment is valid and has no refund path.

On success, the sender receives one whole eNAT whose internal burn amount is `burn(N)`.

There is no artificial burn threshold, era split, supply cap, premine, reservation, or protocol-level allocation.

### 7.1 Required Capture Fee

For a mint transaction included in block `T`, the required capture fee is computed from already indexed blocks before `T`.

Definitions:

```text
window = previous window_blocks blocks before T
median_burn_gwei = median of burn(block) across window
future_mints = accepted mints in window whose target blk >= capture_fee_activation_block
actual_mint_rate_bps = floor(future_mints * 10000 / window_blocks)
raw_multiplier_bps = floor(actual_mint_rate_bps * 10000 / target_mint_rate_bps)
multiplier_bps = clamp(raw_multiplier_bps, min_multiplier_bps, max_multiplier_bps)
required_fee = ceil(median_burn_gwei * multiplier_bps / 10000)
```

If capture fee applies and `required_fee` would be `0`, the required fee is `1` gwei.

The computation is deterministic and uses integer arithmetic only.

## 8. Transfer

Transfer moves either a whole eNAT or a gwei amount from fragment balance.

```json
{
  "p": "edmt",
  "op": "emt-transfer",
  "tick": "enat",
  "amt": "<gwei_amount>",
  "to": "0x0000000000000000000000000000000000000000",
  "src": "<block_number_or_balance>"
}
```

Fields:

| Field | Rule |
| --- | --- |
| `amt` | Decimal positive integer string, unit gwei. |
| `to` | Ethereum address, `0x` plus 40 hex characters. |
| `src` | Either a decimal block number string or `"balance"`. |

The debit source is always the Ethereum transaction sender (`tx.from`). The Ethereum transaction `to` field does not define the protocol recipient.

### 8.1 `src = "<block_number>"`

The sender must hold the whole eNAT for that block.

If `amt == burn(block_number)`:

- the whole eNAT moves from sender to recipient;
- the recipient receives a whole holding with the same `blk` and `burn_amount`.

If `amt < burn(block_number)`:

- the whole eNAT is split;
- the whole holding is removed from the sender;
- `amt` is appended to the recipient's fragment FIFO queue;
- `burn(block_number) - amt` is appended to the sender's fragment FIFO queue.

If `amt > burn(block_number)`, the operation is rejected.

### 8.2 `src = "balance"`

Only the sender's fragment balance may be used.

The sender's fragment FIFO queue is consumed from oldest to newest until `amt` is paid. The recipient receives one new fragment entry of amount `amt` appended to the tail of their FIFO queue.

If the sender's fragment balance is insufficient, the operation is rejected.

### 8.3 No Automatic Whole Detection

The protocol never infers a whole transfer from `amt` alone. Different blocks can have the same burn amount. A whole transfer MUST explicitly use `src = "<block_number>"`.

## 9. Batch Transfer

Batch transfer executes multiple transfers atomically in a single Ethereum transaction.

```json
{
  "p": "edmt",
  "op": "emt-batch-transfer",
  "tick": "enat",
  "items": [
    {
      "amt": "<gwei_amount>",
      "to": "0x0000000000000000000000000000000000000000",
      "src": "<block_number_or_balance>"
    }
  ]
}
```

Rules:

- `items` MUST be a non-empty array.
- `items.length` MUST be less than or equal to `256`.
- Each item follows the same field rules as `emt-transfer`.
- Items execute in array order.
- State changes from earlier items are visible to later items.
- If any item fails, the entire batch is rejected and no protocol state changes.

## 10. Burn

Burn permanently removes a gwei amount from protocol circulation.

```json
{
  "p": "edmt",
  "op": "emt-burn",
  "tick": "enat",
  "amt": "<gwei_amount>",
  "src": "<block_number_or_balance>"
}
```

Rules:

- `amt` MUST be a decimal positive integer string.
- `src` MUST be a decimal block number string or `"balance"`.
- `to` is not part of burn semantics. If present, it MUST be ignored.

If `src = "<block_number>"`:

- `amt == burn(block_number)`: the whole eNAT is destroyed.
- `amt < burn(block_number)`: the whole eNAT is split; the remainder returns to the sender as a fragment; `amt` is destroyed.
- `amt > burn(block_number)`: reject.

If `src = "balance"`:

- the sender's fragment FIFO queue is consumed by `amt`;
- if balance is insufficient, reject;
- consumed amount is destroyed.

A transfer to `0x0000000000000000000000000000000000000000` is not the same as `emt-burn`. A zero-address transfer leaves the amount owned by that address at the protocol layer. `emt-burn` reduces circulating protocol supply.

## 11. First-Is-First

For a given `(tick, blk)`, the first valid mint in canonical Ethereum chain order wins.

Ordering:

1. lower Ethereum block number first;
2. within the same block, lower `transactionIndex` first.

All later valid mints for the same `(tick, blk)` are rejected.

## 12. Finality and Reorgs

Before finality, pending state may be rolled back by a chain reorganization.

After the configured finality threshold, confirmed state is treated as permanent by the protocol indexer. A reorg deeper than finality is considered an exceptional Ethereum-level event outside automatic protocol handling.

Compliant indexers MUST converge on the same confirmed state when fed the same finalized chain history.

## 13. JSON Rules

Compliant indexers MUST apply the following JSON rules:

- Field order does not matter.
- Legal JSON whitespace is accepted.
- Field names and string values are case-sensitive.
- Numeric fields such as `amt` and `blk` MUST be decimal positive integer strings.
- `fee`, when present, MUST be a decimal non-negative integer string.
- Hex numbers, decimals, scientific notation, negative numbers, empty strings, and leading zero forms such as `"007"` are invalid.
- Unknown fields MUST be ignored.
- Unknown operations MUST be ignored.
- A calldata payload containing multiple concatenated JSON objects MUST be rejected.

## 14. Boundary Rules

| Case | Result |
| --- | --- |
| `amt == "0"` | reject |
| `amt` contains non-decimal characters | reject |
| `amt` has leading zeroes | reject |
| `amt > 2^256 - 1` | reject |
| `blk` has non-decimal characters | reject |
| `blk` has leading zeroes | reject |
| `blk < eip1559_activation_block` | reject |
| `blk > current chain head` for mint | reject |
| capture fee required and `fee` is absent | reject |
| capture fee required and `fee < required_fee` | reject |
| capture fee required and sender fragment balance is insufficient | reject |
| capture fee not required and `fee` is absent | accept if all other mint rules pass |
| capture fee not required and `fee == "0"` | accept if all other mint rules pass |
| `to` is not `0x` plus 40 hex characters | reject |
| `tick != "enat"` for canonical eNAT operations | reject |
| transfer `to == tx.from` | reject |
| batch item `to == tx.from` | reject entire batch |
| transfer to zero address | accept as a normal transfer |
| transfer to any other burn-like address | accept as a normal transfer |
| reverted Ethereum transaction | ignore calldata |

## 15. Extensibility

The protocol layer is intentionally narrow.

Applications may add extra fields to known operations. Indexers MUST ignore fields not defined by this specification.

Applications may also define their own operations. If an operation is not one of the five protocol operations, compliant protocol indexers MUST ignore it and MUST NOT apply state changes.

Field names beginning with `_` are reserved for protocol use.

## 16. Outside the Protocol

The following are not part of the base protocol:

- websites;
- wallets;
- explorers;
- application-layer systems;
- API implementations;
- indexer implementation choices;
- infrastructure and operations.

They may evolve, be replaced, or be forked without changing the protocol layer.
