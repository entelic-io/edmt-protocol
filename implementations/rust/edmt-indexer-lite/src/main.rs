use edmt_indexer_lite::{records_for_block, RpcClient};

#[derive(Debug)]
struct Args {
    rpc_url: String,
    from_block: u64,
    to_block: u64,
    include_ignored: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = match Args::parse() {
        Ok(args) => args,
        Err(message) if message == "help" => {
            print_usage();
            return Ok(());
        }
        Err(message) => {
            eprintln!("error: {message}");
            eprintln!();
            print_usage();
            std::process::exit(2);
        }
    };

    let client = RpcClient::new(args.rpc_url);

    for block_number in args.from_block..=args.to_block {
        let Some(block) = client.get_block_by_number(block_number).await? else {
            eprintln!("warning: block {block_number} was not returned by the RPC endpoint");
            continue;
        };

        for record in records_for_block(block_number, &block, args.include_ignored) {
            println!("{}", serde_json::to_string(&record)?);
        }
    }

    Ok(())
}

impl Args {
    fn parse() -> Result<Self, String> {
        let mut rpc_url = std::env::var("EDMT_RPC_URL").ok();
        let mut from_block = None;
        let mut to_block = None;
        let mut include_ignored = false;

        let mut args = std::env::args().skip(1);

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--help" | "-h" => return Err("help".to_owned()),
                "--rpc-url" => rpc_url = Some(next_value(&mut args, "--rpc-url")?),
                "--from-block" => {
                    from_block = Some(parse_block_number(
                        &next_value(&mut args, "--from-block")?,
                        "--from-block",
                    )?)
                }
                "--to-block" => {
                    to_block = Some(parse_block_number(
                        &next_value(&mut args, "--to-block")?,
                        "--to-block",
                    )?)
                }
                "--include-ignored" => include_ignored = true,
                other => return Err(format!("unknown argument: {other}")),
            }
        }

        let rpc_url = rpc_url
            .ok_or_else(|| "missing --rpc-url or EDMT_RPC_URL environment variable".to_owned())?;
        let from_block = from_block.ok_or_else(|| "missing --from-block".to_owned())?;
        let to_block = to_block.ok_or_else(|| "missing --to-block".to_owned())?;

        if from_block > to_block {
            return Err("--from-block must be less than or equal to --to-block".to_owned());
        }

        Ok(Self {
            rpc_url,
            from_block,
            to_block,
            include_ignored,
        })
    }
}

fn next_value(args: &mut impl Iterator<Item = String>, name: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("missing value after {name}"))
}

fn parse_block_number(value: &str, name: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .map_err(|_| format!("{name} must be a decimal unsigned integer"))
}

fn print_usage() {
    eprintln!(
        r#"Usage:
  edmt-indexer-lite --rpc-url <url> --from-block <n> --to-block <n> [--include-ignored]

Environment:
  EDMT_RPC_URL may be used instead of --rpc-url.

Output:
  One JSON object per recognized or rejected transaction, written as JSONL.

Notes:
  This tool scans calldata and parses eDMT envelopes only. It does not maintain
  balances, finality, reorg handling, capture fee accounting, or application
  state."#
    );
}
