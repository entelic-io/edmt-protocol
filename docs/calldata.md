# Calldata Examples

eDMT operations are UTF-8 strings placed in Ethereum transaction calldata.

Every protocol operation begins with:

```text
data:,
```

followed by one JSON object.

## 1. Encoding

Human-readable form:

```text
data:,{"p":"edmt","op":"emt-mint","tick":"enat","blk":"1705479"}
```

The Ethereum transaction `input` field is the UTF-8 bytes of that string, hex-encoded with a `0x` prefix.

Example JavaScript:

```js
const calldata = `data:,{"p":"edmt","op":"emt-mint","tick":"enat","blk":"1705479"}`;
const input = `0x${Buffer.from(calldata, "utf8").toString("hex")}`;
```

## 2. Deploy

```text
data:,{"p":"edmt","op":"emt-deploy","elem":"0x0000000000000000000000000000000000000000000000000000000000000000","tick":"enat","dt":"n"}
```

JSON payload:

```json
{
  "p": "edmt",
  "op": "emt-deploy",
  "elem": "0x0000000000000000000000000000000000000000000000000000000000000000",
  "tick": "enat",
  "dt": "n"
}
```

`elem` is shown as a placeholder. A real deployment references the transaction hash of the burn element registration.

## 3. Mint

```text
data:,{"p":"edmt","op":"emt-mint","tick":"enat","blk":"1705479"}
```

JSON payload:

```json
{
  "p": "edmt",
  "op": "emt-mint",
  "tick": "enat",
  "blk": "1705479"
}
```

The Ethereum transaction sender becomes the owner if the mint is valid and first for that block.

If the target block is at or after `capture_fee_activation_block`, the mint payload MUST include `fee`:

```text
data:,{"p":"edmt","op":"emt-mint","tick":"enat","blk":"1705479","fee":"12000000"}
```

JSON payload:

```json
{
  "p": "edmt",
  "op": "emt-mint",
  "tick": "enat",
  "blk": "1705479",
  "fee": "12000000"
}
```

`fee` is a gwei amount paid from the sender's raw fragment balance and destroyed by the protocol indexer. Before capture fee activation, `fee` may be omitted or set to `"0"`.

## 4. Transfer a Whole eNAT

```text
data:,{"p":"edmt","op":"emt-transfer","tick":"enat","amt":"123456789","to":"0x1111111111111111111111111111111111111111","src":"1705479"}
```

JSON payload:

```json
{
  "p": "edmt",
  "op": "emt-transfer",
  "tick": "enat",
  "amt": "123456789",
  "to": "0x1111111111111111111111111111111111111111",
  "src": "1705479"
}
```

If `amt` equals `burn(1705479)` and the sender owns block `1705479` as a whole holding, the whole eNAT moves to `to`.

## 5. Split a Whole eNAT

```text
data:,{"p":"edmt","op":"emt-transfer","tick":"enat","amt":"1000000","to":"0x2222222222222222222222222222222222222222","src":"1705479"}
```

If `amt` is less than `burn(1705479)`, the whole eNAT is split:

- `1000000` gwei goes to the recipient's fragment FIFO queue;
- the remainder goes to the sender's fragment FIFO queue;
- the original whole holding is removed.

## 6. Transfer from Fragment Balance

```text
data:,{"p":"edmt","op":"emt-transfer","tick":"enat","amt":"500000","to":"0x3333333333333333333333333333333333333333","src":"balance"}
```

JSON payload:

```json
{
  "p": "edmt",
  "op": "emt-transfer",
  "tick": "enat",
  "amt": "500000",
  "to": "0x3333333333333333333333333333333333333333",
  "src": "balance"
}
```

Only the sender's fragment FIFO queue is used. Whole holdings are not split when `src` is `"balance"`.

## 7. Batch Transfer

```text
data:,{"p":"edmt","op":"emt-batch-transfer","tick":"enat","items":[{"amt":"100000","to":"0x4444444444444444444444444444444444444444","src":"balance"},{"amt":"200000","to":"0x5555555555555555555555555555555555555555","src":"1705479"}]}
```

JSON payload:

```json
{
  "p": "edmt",
  "op": "emt-batch-transfer",
  "tick": "enat",
  "items": [
    {
      "amt": "100000",
      "to": "0x4444444444444444444444444444444444444444",
      "src": "balance"
    },
    {
      "amt": "200000",
      "to": "0x5555555555555555555555555555555555555555",
      "src": "1705479"
    }
  ]
}
```

Batch transfer is atomic. If any item fails, no item changes protocol state.

## 8. Burn

```text
data:,{"p":"edmt","op":"emt-burn","tick":"enat","amt":"250000","src":"balance"}
```

JSON payload:

```json
{
  "p": "edmt",
  "op": "emt-burn",
  "tick": "enat",
  "amt": "250000",
  "src": "balance"
}
```

This permanently removes `250000` gwei from the sender's fragment balance and increases `total_burned`.

## 9. Validity Notes

These are valid:

```json
{"p":"edmt","op":"emt-mint","tick":"enat","blk":"1"}
```

```json
{"op":"emt-mint","blk":"1","tick":"enat","p":"edmt"}
```

When capture fee applies, this is valid if `12000000` is greater than or equal to the required fee and the sender has enough raw fragment balance:

```json
{"p":"edmt","op":"emt-mint","tick":"enat","blk":"1","fee":"12000000"}
```

These are invalid:

```json
{"p":"edmt","op":"emt-mint","tick":"ENAT","blk":"1"}
```

```json
{"p":"edmt","op":"emt-transfer","tick":"enat","amt":"001","to":"0x1111111111111111111111111111111111111111","src":"balance"}
```

```json
{"p":"edmt","op":"emt-transfer","tick":"enat","amt":"1.5","to":"0x1111111111111111111111111111111111111111","src":"balance"}
```

```json
{"p":"edmt","op":"emt-transfer","tick":"enat","amt":"1","to":"0x1111111111111111111111111111111111111111"}
```

When capture fee applies, this is invalid because `fee` is missing:

```json
{"p":"edmt","op":"emt-mint","tick":"enat","blk":"1"}
```

Unknown fields are ignored:

```json
{
  "p": "edmt",
  "op": "emt-transfer",
  "tick": "enat",
  "amt": "100",
  "to": "0x1111111111111111111111111111111111111111",
  "src": "balance",
  "memo": "ignored by protocol indexers"
}
```

Unknown operations are ignored by protocol indexers:

```json
{
  "p": "edmt",
  "op": "emt-example-app-operation",
  "tick": "enat"
}
```
