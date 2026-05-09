use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PREFIX: &[u8] = b"data:,";

const KNOWN_OPS: &[&str] = &[
    "emt-deploy",
    "emt-mint",
    "emt-transfer",
    "emt-batch-transfer",
    "emt-burn",
];

#[derive(Debug, Clone, PartialEq)]
pub enum ParsedCalldata {
    Ignored { reason: &'static str },
    Rejected { reason: &'static str },
    Parsed { operation: String, payload: Value },
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanRecord {
    pub block_number: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_hash: Option<String>,
    pub tx_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_index: Option<u64>,
    #[serde(rename = "from", skip_serializing_if = "Option::is_none")]
    pub from_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    pub classification: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RpcBlock {
    pub number: Option<String>,
    pub hash: Option<String>,
    #[serde(default)]
    pub transactions: Vec<RpcTransaction>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RpcTransaction {
    pub hash: String,
    pub from: Option<String>,
    pub to: Option<String>,
    pub input: Option<String>,
    #[serde(rename = "transactionIndex")]
    pub transaction_index: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RpcResponse<T> {
    result: Option<T>,
    error: Option<RpcError>,
}

#[derive(Debug, Deserialize)]
struct RpcError {
    code: i64,
    message: String,
}

pub struct RpcClient {
    url: String,
    http: reqwest::Client,
}

impl RpcClient {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            http: reqwest::Client::new(),
        }
    }

    pub async fn get_block_by_number(
        &self,
        block_number: u64,
    ) -> Result<Option<RpcBlock>, Box<dyn std::error::Error>> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_getBlockByNumber",
            "params": [quantity(block_number), true]
        });

        let response: RpcResponse<RpcBlock> = self
            .http
            .post(&self.url)
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        if let Some(error) = response.error {
            return Err(format!("json-rpc error {}: {}", error.code, error.message).into());
        }

        Ok(response.result)
    }
}

pub fn parse_input(input: &str) -> ParsedCalldata {
    let bytes = match decode_hex_input(input) {
        Ok(bytes) => bytes,
        Err(_) => {
            return ParsedCalldata::Ignored {
                reason: "invalid_hex_input",
            };
        }
    };

    if !bytes.starts_with(PREFIX) {
        return ParsedCalldata::Ignored {
            reason: "missing_prefix",
        };
    }

    let payload_bytes = &bytes[PREFIX.len()..];
    let payload_text = match std::str::from_utf8(payload_bytes) {
        Ok(text) => text,
        Err(_) => {
            return ParsedCalldata::Rejected {
                reason: "payload_not_utf8",
            };
        }
    };

    let value = match parse_single_json(payload_text) {
        Ok(value) => value,
        Err(reason) => return ParsedCalldata::Rejected { reason },
    };

    let Some(object) = value.as_object() else {
        return ParsedCalldata::Rejected {
            reason: "payload_not_object",
        };
    };

    if object.get("p").and_then(Value::as_str) != Some("edmt") {
        return ParsedCalldata::Ignored {
            reason: "not_edmt_protocol",
        };
    }

    let Some(operation) = object.get("op").and_then(Value::as_str) else {
        return ParsedCalldata::Rejected {
            reason: "missing_operation",
        };
    };

    if !KNOWN_OPS.contains(&operation) {
        return ParsedCalldata::Ignored {
            reason: "unknown_operation",
        };
    }

    ParsedCalldata::Parsed {
        operation: operation.to_owned(),
        payload: value,
    }
}

pub fn records_for_block(
    requested_block: u64,
    block: &RpcBlock,
    include_ignored: bool,
) -> Vec<ScanRecord> {
    let block_number = block
        .number
        .as_deref()
        .and_then(quantity_to_u64)
        .unwrap_or(requested_block);

    let mut records = Vec::new();

    for tx in &block.transactions {
        let input = tx.input.as_deref().unwrap_or("0x");
        let parsed = parse_input(input);

        if matches!(parsed, ParsedCalldata::Ignored { .. }) && !include_ignored {
            continue;
        }

        let (classification, reason, operation, payload) = match parsed {
            ParsedCalldata::Ignored { reason } => ("ignored", Some(reason), None, None),
            ParsedCalldata::Rejected { reason } => ("rejected", Some(reason), None, None),
            ParsedCalldata::Parsed { operation, payload } => {
                ("parsed", None, Some(operation), Some(payload))
            }
        };

        records.push(ScanRecord {
            block_number,
            block_hash: block.hash.clone(),
            tx_hash: tx.hash.clone(),
            tx_index: tx.transaction_index.as_deref().and_then(quantity_to_u64),
            from_address: tx.from.clone(),
            to: tx.to.clone(),
            classification,
            reason,
            operation,
            payload,
        });
    }

    records
}

pub fn quantity(value: u64) -> String {
    format!("0x{value:x}")
}

pub fn quantity_to_u64(value: &str) -> Option<u64> {
    let hex = value.strip_prefix("0x")?;
    u64::from_str_radix(hex, 16).ok()
}

fn parse_single_json(input: &str) -> Result<Value, &'static str> {
    let mut stream = serde_json::Deserializer::from_str(input).into_iter::<Value>();

    let value = match stream.next() {
        Some(Ok(value)) => value,
        Some(Err(_)) => return Err("malformed_json"),
        None => return Err("empty_json_payload"),
    };

    match stream.next() {
        Some(Ok(_)) => Err("multiple_json_values"),
        Some(Err(_)) => Err("trailing_json_data"),
        None => Ok(value),
    }
}

fn decode_hex_input(input: &str) -> Result<Vec<u8>, &'static str> {
    let hex = input.strip_prefix("0x").ok_or("missing_hex_prefix")?;
    if hex.len() % 2 != 0 {
        return Err("odd_hex_length");
    }

    let mut out = Vec::with_capacity(hex.len() / 2);
    let bytes = hex.as_bytes();

    for pair in bytes.chunks_exact(2) {
        let hi = hex_nibble(pair[0]).ok_or("invalid_hex_digit")?;
        let lo = hex_nibble(pair[1]).ok_or("invalid_hex_digit")?;
        out.push((hi << 4) | lo);
    }

    Ok(out)
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(text: &str) -> String {
        let mut hex = String::from("0x");
        for byte in text.as_bytes() {
            hex.push_str(&format!("{byte:02x}"));
        }
        hex
    }

    #[test]
    fn parses_known_protocol_operation() {
        let parsed = parse_input(&input(
            r#"data:,{"p":"edmt","op":"emt-mint","tick":"enat","blk":"1705479"}"#,
        ));

        match parsed {
            ParsedCalldata::Parsed { operation, payload } => {
                assert_eq!(operation, "emt-mint");
                assert_eq!(payload["tick"], "enat");
            }
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn ignores_non_protocol_input() {
        let parsed = parse_input("0x1234");
        assert_eq!(
            parsed,
            ParsedCalldata::Ignored {
                reason: "missing_prefix"
            }
        );
    }

    #[test]
    fn rejects_malformed_protocol_json() {
        let parsed = parse_input(&input(r#"data:,{"p":"edmt","op":"emt-mint""#));
        assert_eq!(
            parsed,
            ParsedCalldata::Rejected {
                reason: "malformed_json"
            }
        );
    }

    #[test]
    fn ignores_unknown_operation() {
        let parsed = parse_input(&input(r#"data:,{"p":"edmt","op":"emt-future"}"#));
        assert_eq!(
            parsed,
            ParsedCalldata::Ignored {
                reason: "unknown_operation"
            }
        );
    }

    #[test]
    fn rejects_multiple_json_values() {
        let parsed = parse_input(&input(r#"data:,{"p":"edmt","op":"emt-mint"}{"p":"edmt"}"#));
        assert_eq!(
            parsed,
            ParsedCalldata::Rejected {
                reason: "multiple_json_values"
            }
        );
    }

    #[test]
    fn formats_and_parses_quantities() {
        assert_eq!(quantity(12_965_000), "0xc5d488");
        assert_eq!(quantity_to_u64("0xc5d488"), Some(12_965_000));
    }
}
