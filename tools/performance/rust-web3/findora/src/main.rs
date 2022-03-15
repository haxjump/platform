use clap::{Parser, Subcommand};
use std::cell::RefCell;
use std::cmp::Ordering;
use std::ops::{Mul, MulAssign, Sub};
use std::str::FromStr;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use feth::{one_eth_key, utils::*, KeyPair, TestClient, BLOCK_TIME};
use rayon::prelude::*;
use web3::types::{Address, Block, BlockId, BlockNumber, TransactionId, H256, U64};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about=None)]
struct Cli {
    /// The minimum parallelism
    #[clap(long, default_value_t = 1)]
    min_parallelism: u64,

    /// The maximum parallelism
    #[clap(long, default_value_t = 200)]
    max_parallelism: u64,

    /// The count of transactions sent by a routine
    #[clap(long, default_value_t = 0)]
    count: u64,

    /// the source account file
    #[clap(long, parse(from_os_str), value_name = "FILE", default_value = "source_keys.001")]
    source: PathBuf,

    /// block time of the network
    #[clap(long, default_value_t = BLOCK_TIME)]
    block_time: u64,

    /// findora network full-node urls: http://node0:8545,http://node1:8545
    #[clap(long)]
    network: Option<String>,

    /// http request timeout, seconds
    #[clap(long)]
    timeout: Option<u64>,

    #[clap(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Fund Ethereum accounts
    Fund {
        /// ethereum-compatible network
        #[clap(long)]
        network: String,

        /// http request timeout, seconds
        #[clap(long)]
        timeout: Option<u64>,

        /// block time of the network
        #[clap(long, default_value_t = BLOCK_TIME)]
        block_time: u64,

        /// the number of Eth Account to be fund
        #[clap(long, default_value_t = 0)]
        count: u64,

        /// how much 0.1-eth to fund
        #[clap(long, default_value_t = 1)]
        amount: u64,

        /// load keys from file
        #[clap(long)]
        load: bool,

        /// re-deposit account with insufficient balance
        #[clap(long)]
        redeposit: bool,
    },
    /// check ethereum account information
    Info {
        /// ethereum-compatible network
        #[clap(long)]
        network: String,

        /// http request timeout, seconds
        #[clap(long)]
        timeout: Option<u64>,

        /// ethereum address
        #[clap(long)]
        account: Address,
    },

    /// Transaction Operations
    Transaction {
        /// ethereum-compatible network
        #[clap(long)]
        network: String,

        /// http request timeout, seconds
        #[clap(long)]
        timeout: Option<u64>,

        /// transaction hash
        #[clap(long)]
        hash: H256,
    },

    /// Block Operations
    Block {
        /// ethereum-compatible network
        #[clap(long)]
        network: String,

        /// http request timeout, seconds
        #[clap(long)]
        timeout: Option<u64>,

        /// start block height
        #[clap(long)]
        start: Option<u64>,

        /// block count, could be less than zero
        #[clap(long)]
        count: Option<i64>,
    },
}

fn check_parallel_args(max_par: u64, min_par: u64) {
    if max_par > log_cpus() * 1000 {
        panic!(
            "Two much working thread, maybe overload the system {}/{}",
            max_par,
            log_cpus(),
        )
    }
    if max_par < min_par || min_par == 0 || max_par == 0 {
        panic!("Invalid parallel parameters: max {}, min {}", max_par, min_par);
    }
}

fn calc_pool_size(keys: usize, max_par: usize, min_par: usize) -> usize {
    let mut max_pool_size = keys * 2;
    if max_pool_size > max_par {
        max_pool_size = max_par;
    }
    if max_pool_size < min_par {
        max_pool_size = min_par;
    }

    max_pool_size
}

fn eth_transaction(network: &str, timeout: Option<u64>, hash: H256) {
    let network = real_network(network);
    // use first endpoint to fund accounts
    let client = TestClient::setup(network[0].clone(), timeout);
    let tx = client.transaction(TransactionId::from(hash));
    println!("{:?}", tx);
}

fn eth_account(network: &str, timeout: Option<u64>, account: Address) {
    let network = real_network(network);
    // use first endpoint to fund accounts
    let client = TestClient::setup(network[0].clone(), timeout);
    let balance = client.balance(account, None);
    let nonce = client.nonce(account, None);
    println!("{:?}: {} {:?}", account, balance, nonce);
}

fn eth_blocks(network: &str, timeout: Option<u64>, start: Option<u64>, count: Option<i64>) {
    let network = real_network(network);
    // use first endpoint to fund accounts
    let client = TestClient::setup(network[0].clone(), timeout);
    if let Some(start) = start {
        let range = count
            .map(|c| match c.cmp(&0i64) {
                Ordering::Equal => start..start + 1,
                Ordering::Less => {
                    let n = c.abs() as u64;
                    if start > n {
                        start - n..start + 1
                    } else {
                        0..start + 1
                    }
                }
                Ordering::Greater => start..start + c.abs() as u64 + 1,
            })
            .unwrap_or_else(|| match client.block_number() {
                Some(end) if start > end.as_u64() => {
                    panic!(
                        "start block height is bigger than latest height({}>{})",
                        start,
                        end.as_u64()
                    );
                }
                Some(end) => start..end.as_u64() + 1,
                None => panic!("Failed to obtain block height"),
            });
        let last_block: RefCell<Option<(u64, Block<H256>)>> = RefCell::new(if range.start == 0 {
            None
        } else {
            let id = BlockId::Number(BlockNumber::Number(U64::from(range.start - 1)));
            Some((range.start - 1, client.block_with_tx_hashes(id).unwrap()))
        });
        range
            .map(|number| {
                let id = BlockId::Number(BlockNumber::Number(U64::from(number)));
                client.block_with_tx_hashes(id).map(|block| {
                    let block_time = match &*last_block.borrow() {
                        Some(last) if last.0 + 1 == number => (block.timestamp - last.1.timestamp).as_u64(),
                        _ => 0u64,
                    };
                    let count = block.transactions.len();
                    let timestamp = block.timestamp;
                    *last_block.borrow_mut() = Some((number, block));
                    (number, timestamp, count, block_time)
                })
            })
            .for_each(|block| {
                let msg = if let Some(block) = block {
                    format!("{},{:?},{},{}", block.0, block.1, block.2, block.3)
                } else {
                    "None".to_string()
                };
                println!("{}", msg);
            });
    } else if let Some(b) = client.current_block() {
        let block_time = match b.number {
            Some(n) if n > U64::zero() => {
                if let Some(last) = client.block_with_tx_hashes(BlockId::Number(BlockNumber::Number(n.sub(1)))) {
                    Some(b.timestamp - last.timestamp)
                } else {
                    None
                }
            }
            _ => None,
        };
        println!(
            "{},{:?},{},{}",
            b.number.unwrap_or_default(),
            b.timestamp,
            b.transactions.len(),
            block_time.unwrap_or_default(),
        );
    } else {
        println!("Cannot obtain current block");
    }
}

fn fund_accounts(
    network: &str,
    timeout: Option<u64>,
    block_time: u64,
    count: u64,
    am: u64,
    load: bool,
    redeposit: bool,
) {
    let mut amount = web3::types::U256::exp10(17); // 0.1 eth
    amount.mul_assign(am);

    let network = real_network(network);
    // use first endpoint to fund accounts
    let client = TestClient::setup(network[0].clone(), timeout);
    let balance = client.balance(client.root_addr, None);
    println!("Balance of {:?}: {}", client.root_addr, balance);

    let mut source_keys = if load {
        let keys: Vec<_> = serde_json::from_str(std::fs::read_to_string("source_keys.001").unwrap().as_str()).unwrap();
        keys
    } else {
        // check if the key file exists
        println!("generating new source keys");
        if std::fs::File::open("source_keys.001").is_ok() {
            panic!("file \"source_keys.001\" already exists");
        }
        if amount.mul(count + 1) >= balance {
            panic!("Too large source account number, maximum {}", balance / amount);
        }
        let source_keys = (0..count).map(|_| one_eth_key()).collect::<Vec<_>>();
        let data = serde_json::to_string(&source_keys).unwrap();
        std::fs::write("source_keys.001", &data).unwrap();

        source_keys
    };

    // add more source keys and save them to file
    if count as usize > source_keys.len() {
        source_keys.resize_with(count as usize, one_eth_key);

        std::fs::rename("source_keys.001", ".source_keys.001.bak").unwrap();
        let data = serde_json::to_string(&source_keys).unwrap();
        std::fs::write("source_keys.001", &data).unwrap();
    }

    let total = source_keys.len();
    let source_accounts = source_keys
        .into_iter()
        .enumerate()
        .filter_map(|(idx, key)| {
            let from = Address::from_str(key.address.as_str()).unwrap();
            let account = if redeposit {
                let balance = client.balance(from, None);
                if balance < amount {
                    Some((from, amount))
                } else {
                    None
                }
            } else {
                Some((from, amount))
            };
            if let Some(a) = account.as_ref() {
                println!("{}/{} {:?}", idx + 1, total, a);
            }
            account
        })
        .collect::<Vec<_>>();
    // 1000 eth
    let metrics = client
        .distribution(1, None, &source_accounts, &Some(block_time), true)
        .unwrap();
    // save metrics to file
    let data = serde_json::to_string(&metrics).unwrap();
    std::fs::write("metrics.001", &data).unwrap();
}

fn main() -> web3::Result<()> {
    let cli = Cli::parse();

    println!("{:?}", cli);

    match &cli.command {
        Some(Commands::Fund {
            network,
            timeout,
            block_time,
            count,
            amount,
            load,
            redeposit,
        }) => {
            fund_accounts(
                network.as_ref(),
                *timeout,
                *block_time,
                *count,
                *amount,
                *load,
                *redeposit,
            );
            return Ok(());
        }
        Some(Commands::Info {
            network,
            timeout,
            account,
        }) => {
            eth_account(network.as_ref(), *timeout, *account);
            return Ok(());
        }
        Some(Commands::Transaction { network, timeout, hash }) => {
            eth_transaction(network.as_ref(), *timeout, *hash);
            return Ok(());
        }
        Some(Commands::Block {
            network,
            timeout,
            start,
            count,
        }) => {
            eth_blocks(network.as_ref(), *timeout, *start, *count);
            return Ok(());
        }
        None => {}
    }

    let count = cli.count;
    let min_par = cli.min_parallelism;
    let max_par = cli.max_parallelism;
    let timeout = cli.timeout;
    let source_file = cli.source;
    let block_time = Some(cli.block_time);
    let source_keys: Vec<KeyPair> =
        serde_json::from_str(std::fs::read_to_string(source_file).unwrap().as_str()).unwrap();
    let target_amount = web3::types::U256::exp10(16); // 0.01 eth

    println!("logical cpus {}, physical cpus {}", log_cpus(), phy_cpus());
    check_parallel_args(max_par, min_par);

    let max_pool_size = calc_pool_size(source_keys.len(), max_par as usize, min_par as usize);
    rayon::ThreadPoolBuilder::new()
        .num_threads(max_pool_size)
        .build_global()
        .unwrap();
    println!("thread pool size {}", max_pool_size);

    let networks = cli.network.map(|n| real_network(n.as_str()));
    let clients = if let Some(endpoints) = networks {
        endpoints
            .into_iter()
            .map(|n| Arc::new(TestClient::setup(n, timeout)))
            .collect::<Vec<_>>()
    } else {
        vec![Arc::new(TestClient::setup(None, timeout))]
    };
    let client = clients[0].clone();

    println!("chain_id:     {}", client.chain_id().unwrap());
    println!("gas_price:    {}", client.gas_price().unwrap());
    println!("block_number: {}", client.block_number().unwrap());
    println!("frc20 code:   {:?}", client.frc20_code().unwrap());

    println!("preparing test data...");
    let source_keys = source_keys
        .par_iter()
        .filter_map(|kp| {
            let (secret, address) = (
                secp256k1::SecretKey::from_str(kp.private.as_str()).unwrap(),
                Address::from_str(kp.address.as_str()).unwrap(),
            );
            let balance = client.balance(address, None);
            if balance <= target_amount.mul(count) {
                None
            } else {
                let target = (0..count)
                    .map(|_| {
                        (
                            Address::from_str(one_eth_key().address.as_str()).unwrap(),
                            target_amount,
                        )
                    })
                    .collect::<Vec<_>>();
                println!("account {:?} added to source pool", address);
                Some(((secret, address), target))
            }
        })
        .collect::<Vec<_>>();

    if min_par == 0 || count == 0 || source_keys.is_empty() {
        println!("Not enough sufficient source accounts or target accounts, skipped.");
        return Ok(());
    }

    let total_succeed = Arc::new(Mutex::new(0u64));
    let concurrences = if source_keys.len() > max_pool_size {
        max_pool_size
    } else {
        source_keys.len()
    };

    // split the source keys
    let mut chunk_size = source_keys.len() / clients.len();
    if source_keys.len() % clients.len() != 0 {
        chunk_size += 1;
    }

    // one-thread per source key
    // fix one source key to one endpoint

    println!("starting tests...");
    let total = source_keys.len() * count as usize;
    let now = std::time::Instant::now();
    let metrics = source_keys
        .par_chunks(chunk_size)
        .zip(clients)
        .into_par_iter()
        .enumerate()
        .map(|(chunk, (sources, client))| {
            sources
                .into_par_iter()
                .enumerate()
                .map(|(i, (source, targets))| {
                    let metrics = client
                        .distribution(i + 1, Some(*source), targets, &block_time, false)
                        .unwrap();
                    let mut num = total_succeed.lock().unwrap();
                    *num += metrics.succeed;
                    (chunk, i, metrics)
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    let elapsed = now.elapsed().as_secs();

    println!("saving test files");
    metrics.into_iter().for_each(|m| {
        m.into_iter().for_each(|(chunk, i, metrics)| {
            let file = format!("metrics.target.{}.{}", chunk, i);
            let data = serde_json::to_string(&metrics).unwrap();
            std::fs::write(&file, data).unwrap();
        })
    });

    let avg = total as f64 / elapsed as f64;
    println!(
        "Performed {} transfers, max concurrences {}, {:.3} Transfer/s, total {} seconds",
        total, concurrences, avg, elapsed,
    );

    Ok(())
}
