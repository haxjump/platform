use url::Url;

pub fn log_cpus() -> u64 {
    num_cpus::get() as u64
}

pub fn phy_cpus() -> u64 {
    num_cpus::get_physical() as u64
}

pub fn real_network(network: &str) -> Vec<Option<String>> {
    match network {
        "local" => vec![Some("http://localhost:8545".to_string())],
        "anvil" => vec![Some("https://prod-testnet.prod.findora.org:8545".to_string())],
        "mock" => vec![Some("https://dev-mainnetmock.dev.findora.org:8545".to_string())],
        "test" => vec![Some("http://18.236.205.22:8545".to_string())],
        n => {
            // comma seperated network endpoints
            n.split(',')
                .filter_map(|s| {
                    let ns = s.trim();
                    if ns.is_empty() || Url::parse(ns).is_err() {
                        None
                    } else {
                        Some(Some(ns.to_string()))
                    }
                })
                .collect::<Vec<_>>()
        }
    }
}
