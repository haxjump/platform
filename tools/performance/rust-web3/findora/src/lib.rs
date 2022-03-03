pub mod utils;

use crate::utils::extract_keypair_from_file;
use bip0039::{Count, Language, Mnemonic};
use bip32::{DerivationPath, XPrv};
use libsecp256k1::{PublicKey, SecretKey};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};
use std::cell::RefCell;
use std::ops::AddAssign;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;
use web3::{
    transports::Http,
    types::{
        Address, BlockNumber, Bytes, Transaction, TransactionId, TransactionParameters, TransactionReceipt, H160, H256,
        U256, U64,
    },
};

const FRC20_ADDRESS: u64 = 0x1000;
pub const BLOCK_TIME: u64 = 16;

//const WEB3_SRV: &str = "http://127.0.0.1:8545";
//const WEB3_SRV: &str = "http://18.236.205.22:8545";
const WEB3_SRV: &str = "https://prod-testnet.prod.findora.org:8545";
//const WEB3_SRV: &str = "https://dev-mainnetmock.dev.findora.org:8545";

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct KeyPair {
    pub address: String,
    pub private: String,
}

#[inline(always)]
pub fn one_eth_key() -> KeyPair {
    let mnemonic = Mnemonic::generate_in(Language::English, Count::Words12);
    let bs = mnemonic.to_seed("");
    let ext = XPrv::derive_from_path(&bs, &DerivationPath::from_str("m/44'/60'/0'/0/0").unwrap()).unwrap();

    let secret = SecretKey::parse_slice(&ext.to_bytes()).unwrap();
    let public = PublicKey::from_secret_key(&secret);

    let mut res = [0u8; 64];
    res.copy_from_slice(&public.serialize()[1..65]);
    let public = H160::from(H256::from_slice(Keccak256::digest(&res).as_slice()));

    KeyPair {
        address: eth_checksum::checksum(&format!("{:?}", public)),
        private: hex::encode(secret.serialize()),
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct TxMetric {
    pub to: Address,
    pub amount: U256,
    pub hash: Option<H256>, // Tx hash
    pub status: u64,        // 1 - success, other - fail
    pub wait: u64,          // seconds for waiting tx receipt
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct TransferMetrics {
    pub from: Address,
    pub total: u64,
    pub succeed: u64,
    pub txs: Vec<TxMetric>,
}

#[derive(Debug)]
pub struct TestClient {
    pub web3: Arc<web3::Web3<Http>>,
    pub eth: Arc<web3::api::Eth<Http>>,
    pub accounts: Arc<web3::api::Accounts<Http>>,
    pub root_sk: secp256k1::SecretKey,
    pub root_addr: Address,
    rt: Runtime,
}

#[derive(Debug)]
pub struct NetworkInfo {
    pub chain_id: U256,
    pub block_number: U64,
    pub gas_price: U256,
    pub frc20_code: Option<Bytes>,
}

impl TestClient {
    pub fn setup(url: Option<String>) -> Self {
        let transport = web3::transports::Http::new(url.unwrap_or_else(|| WEB3_SRV.to_string()).as_str()).unwrap();
        let web3 = Arc::new(web3::Web3::new(transport));
        let eth = Arc::new(web3.eth());
        let accounts = Arc::new(web3.accounts());
        let (root_sk, root_addr) = extract_keypair_from_file(".secret");
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        Self {
            web3,
            eth,
            accounts,
            root_sk,
            root_addr,
            rt,
        }
    }

    pub fn chain_id(&self) -> Option<U256> {
        self.rt.block_on(self.eth.chain_id()).ok()
    }

    pub fn block_number(&self) -> Option<U64> {
        self.rt.block_on(self.eth.block_number()).ok()
    }

    pub fn nonce(&self, from: Address, block: Option<BlockNumber>) -> Option<U256> {
        self.rt.block_on(self.eth.transaction_count(from, block)).ok()
    }

    pub fn gas_price(&self) -> Option<U256> {
        self.rt.block_on(self.eth.gas_price()).ok()
    }

    pub fn frc20_code(&self) -> Option<Bytes> {
        self.rt
            .block_on(self.eth.code(H160::from_low_u64_be(FRC20_ADDRESS), None))
            .ok()
    }

    #[allow(unused)]
    pub fn transaction(&self, id: TransactionId) -> Option<Transaction> {
        self.rt.block_on(self.eth.transaction(id)).unwrap_or_default()
    }

    pub fn transaction_receipt(&self, hash: H256) -> Option<TransactionReceipt> {
        self.rt.block_on(self.eth.transaction_receipt(hash)).unwrap_or_default()
    }

    #[allow(unused)]
    pub fn accounts(&self) -> Vec<Address> {
        self.rt.block_on(self.eth.accounts()).unwrap_or_default()
    }

    pub fn balance(&self, address: Address, number: Option<BlockNumber>) -> U256 {
        self.rt.block_on(self.eth.balance(address, number)).unwrap_or_default()
    }

    pub fn wait_for_tx_receipt(&self, hash: H256, interval: Duration, times: u64) -> (u64, Option<TransactionReceipt>) {
        let mut wait = 0;
        let mut retry = times;
        loop {
            if let Some(receipt) = self.transaction_receipt(hash) {
                wait = times + 1 - retry;
                break (wait, Some(receipt));
            } else {
                std::thread::sleep(interval);
                retry -= 1;
                if retry == 0 {
                    break (wait, None);
                }
            }
        }
    }

    pub fn distribution(
        &self,
        source: Option<(secp256k1::SecretKey, Address)>,
        targets: &[(Address, U256)],
        block_time: &Option<u64>,
    ) -> web3::Result<TransferMetrics> {
        let mut results = vec![];
        let mut succeed = 0u64;
        let total = targets.len();
        let source_address = source.unwrap_or((self.root_sk, self.root_addr)).1;
        let source_sk = source.unwrap_or((self.root_sk, self.root_addr)).0;
        let wait_time = block_time.unwrap_or(BLOCK_TIME) * 3 + 1;
        let chain_id = self.chain_id().map(|id| id.as_u64());
        let gas_price = self.gas_price();
        let nonce = RefCell::new(self.nonce(source_address, Some(BlockNumber::Pending)).unwrap());
        targets
            .iter()
            .map(|(account, am)| {
                let to = Some(*account);
                let tm = TxMetric {
                    to: to.unwrap(),
                    amount: *am,
                    status: 99,
                    ..Default::default()
                };
                let tp = TransactionParameters {
                    to,
                    value: *am,
                    chain_id,
                    gas_price,
                    nonce: Some(*nonce.borrow()),
                    ..Default::default()
                };
                (tp, tm)
            })
            .enumerate()
            // Sign the txs (can be done offline)
            .for_each(|(idx, (mut tx_object, mut metric))| {
                match self
                    .rt
                    .block_on(self.accounts.sign_transaction(tx_object.clone(), &source_sk))
                {
                    Ok(signed) => match self.rt.block_on(self.eth.send_raw_transaction(signed.raw_transaction)) {
                        Ok(hash) => {
                            metric.hash = Some(hash);
                            let (wait, receipt) =
                                self.wait_for_tx_receipt(hash, Duration::from_millis(100), wait_time * 10);
                            match receipt {
                                Some(r) if r.status == Some(U64::one()) => {
                                    succeed += 1;
                                    metric.status = 1;
                                }
                                _ => {}
                            }
                            metric.wait = wait;
                            println!("{}/{} {:?} {:?}", idx + 1, total, metric.to, hash);
                            nonce.borrow_mut().add_assign(U256::one());
                        }
                        Err(e) => {
                            metric.status = 97;
                            // retrieve nonce if failed to send tx
                            *nonce.borrow_mut() = self.nonce(source_address, Some(BlockNumber::Pending)).unwrap();
                            // give it another chance
                            tx_object.nonce = Some(*nonce.borrow());
                            if let Ok(signed) = self.rt.block_on(self.accounts.sign_transaction(tx_object, &source_sk))
                            {
                                match self.rt.block_on(self.eth.send_raw_transaction(signed.raw_transaction)) {
                                    Ok(hash) => {
                                        metric.hash = Some(hash);
                                        let (wait, receipt) =
                                            self.wait_for_tx_receipt(hash, Duration::from_millis(100), wait_time * 10);
                                        match receipt {
                                            Some(r) if r.status == Some(U64::one()) => {
                                                succeed += 1;
                                                metric.status = 1;
                                            }
                                            _ => {}
                                        }
                                        metric.wait = wait;
                                        println!("retry {}/{} {:?} {:?}", idx + 1, total, metric.to, hash);
                                        nonce.borrow_mut().add_assign(U256::one());
                                    }
                                    Err(e) => {
                                        println!("give up {}/{} {:?} {:?}", idx + 1, total, metric.to, e);
                                        *nonce.borrow_mut() =
                                            self.nonce(source_address, Some(BlockNumber::Pending)).unwrap();
                                    }
                                }
                            } else {
                                println!("give up {}/{} {:?} {:?}", idx + 1, total, metric.to, e);
                                *nonce.borrow_mut() = self.nonce(source_address, Some(BlockNumber::Pending)).unwrap();
                            }
                        }
                    },
                    Err(e) => {
                        println!("{}/{} {:?} {:?}", idx + 1, total, metric.to, e);
                        metric.status = 98;
                        // retrieve nonce if failed to send tx
                        *nonce.borrow_mut() = self.nonce(source_address, Some(BlockNumber::Pending)).unwrap();
                    }
                }

                results.push(metric);
            });

        //println!("Waiting for final results...");

        //results.iter_mut().enumerate().for_each(|(idx, metric)| {
        //    let mut retry = wait_time;
        //    loop {
        //        if let Some(hash) = metric.hash {
        //            if let Some(receipt) = self.transaction_receipt(hash) {
        //                if let Some(status) = receipt.status {
        //                    if status == U64::from(1u64) {
        //                        succeed += 1;
        //                        metric.status = 1;
        //                    }
        //                }
        //                metric.wait = wait_time + 1 - retry;
        //                break;
        //            } else {
        //                std::thread::sleep(Duration::from_secs(1));
        //                retry -= 1;
        //                if retry == 0 {
        //                    metric.wait = wait_time;
        //                    break;
        //                }
        //            }
        //        }
        //    }
        //    println!(
        //        "{}/{} {:?} {:?} {}",
        //        idx,
        //        total,
        //        metric.to,
        //        metric.hash,
        //        metric.status == 1
        //    );
        //});

        println!("Tx succeeded: {}/{}", succeed, total);

        Ok(TransferMetrics {
            from: source_address,
            total: targets.len() as u64,
            succeed,
            txs: results,
        })
    }
}
