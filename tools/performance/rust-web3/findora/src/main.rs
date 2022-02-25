use clap::{Parser, Subcommand};
use std::ops::{Mul, MulAssign};
use std::str::FromStr;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use feth::{one_eth_key, utils::*, KeyPair, TestClient, BLOCK_TIME, ROOT_ADDR};
use rayon::prelude::*;
use web3::types::Address;

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

    /// findora network fullnode urls: http://path:8545,http://path1:8546
    #[clap(long)]
    network: Option<String>,

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
    },
}

fn check_parallel_args(max_par: u64, min_par: u64) {
    if max_par > log_cpus() * 100 {
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

fn fund_accounts(network: &str, block_time: u64, mut count: u64, am: u64, load: bool) {
    let mut amount = web3::types::U256::exp10(17); // 0.1 eth
    amount.mul_assign(am);

    let network = real_network(network);
    // use first endpoint to fund accounts
    let client = TestClient::setup(network[0].clone(), None, None);
    let balance = client.balance(ROOT_ADDR[2..].parse().unwrap(), None);
    println!("Root Balance: {}", balance);

    let source_keys = if load {
        let keys: Vec<_> = serde_json::from_str(std::fs::read_to_string("source_keys.001").unwrap().as_str()).unwrap();
        count = keys.len() as u64;
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

    let source_accounts = source_keys
        .into_iter()
        .map(|key| Address::from_str(key.address.as_str()).unwrap())
        .collect::<Vec<_>>();
    // 1000 eth
    let amounts = vec![amount; count as usize];
    let metrics = client
        .distribution(None, &source_accounts, &amounts, &Some(block_time))
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
            block_time,
            count,
            amount,
            load,
        }) => {
            fund_accounts(network.as_ref(), *block_time, *count, *amount, *load);
            return Ok(());
        }
        None => {}
    }

    let per_count = cli.count;
    let min_par = cli.min_parallelism;
    let max_par = cli.max_parallelism;
    let source_file = cli.source;
    let _prog = "feth".to_owned();
    let block_time = Some(cli.block_time);
    let source_keys: Vec<KeyPair> =
        serde_json::from_str(std::fs::read_to_string(source_file).unwrap().as_str()).unwrap();
    let target_amount = web3::types::U256::exp10(17); // 0.1 eth

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
            .map(|n| Arc::new(TestClient::setup(n, None, None)))
            .collect::<Vec<_>>()
    } else {
        vec![Arc::new(TestClient::setup(None, None, None))]
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
            let balance = client.balance(kp.address[2..].parse().unwrap(), None);
            if balance <= target_amount.mul(per_count) {
                None
            } else {
                Some(kp)
            }
        })
        .map(|m| {
            (
                (
                    secp256k1::SecretKey::from_str(m.private.as_str()).unwrap(),
                    Address::from_str(m.address.as_str()).unwrap(),
                ),
                (0..per_count)
                    .map(|_| Address::from_str(one_eth_key().address.as_str()).unwrap())
                    .collect::<Vec<_>>(),
                vec![target_amount; per_count as usize],
            )
        })
        .collect::<Vec<_>>();

    if min_par == 0 || per_count == 0 || source_keys.is_empty() {
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
    // channel1 {endpoint, from, tx_hash}: tx_sender -> tx_producer
    // channel2 {endpoint, from, target}: tx_producer -> tx_sender
    // tx_producer: check previous tx hash, create tx, send to channel2
    // tx_sender: send tx, send tx hash to channel1

    println!("starting tests...");
    let total = source_keys.len() * per_count as usize;
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
                .map(|(i, (source, accounts, amounts))| {
                    let metrics = client
                        .distribution(Some(*source), accounts, amounts, &block_time)
                        .unwrap();
                    //let file = format!("metrics.target.{}.{}", chunk, i);
                    //let data = serde_json::to_string(&metrics).unwrap();
                    //std::fs::write(&file, data).unwrap();

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
        "Performed {} transfers, max concurrences {}, succeed {}, {:.3} Transfer/s, total {} seconds",
        total,
        concurrences,
        total_succeed.lock().unwrap(),
        avg,
        elapsed,
    );

    Ok(())
}
