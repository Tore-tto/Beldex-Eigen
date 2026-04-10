use libp2p::rendezvous::Namespace;
use std::fmt;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum BeldexBtcNamespace {
    Mainnet,
    Testnet,
}

const MAINNET: &str = "bdx-btc-swap-mainnet";
const TESTNET: &str = "bdx-btc-swap-testnet";

impl fmt::Display for BeldexBtcNamespace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BeldexBtcNamespace::Mainnet => write!(f, "{}", MAINNET),
            BeldexBtcNamespace::Testnet => write!(f, "{}", TESTNET),
        }
    }
}

impl From<BeldexBtcNamespace> for Namespace {
    fn from(namespace: BeldexBtcNamespace) -> Self {
        match namespace {
            BeldexBtcNamespace::Mainnet => Namespace::from_static(MAINNET),
            BeldexBtcNamespace::Testnet => Namespace::from_static(TESTNET),
        }
    }
}

impl BeldexBtcNamespace {
    pub fn from_is_testnet(testnet: bool) -> BeldexBtcNamespace {
        if testnet {
            BeldexBtcNamespace::Testnet
        } else {
            BeldexBtcNamespace::Mainnet
        }
    }
}
