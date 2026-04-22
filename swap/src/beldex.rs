pub mod wallet;
mod wallet_rpc;

pub use ::beldex::network::Network;
pub use ::beldex::{Address, PrivateKey, PublicKey};
pub use curve25519_dalek_ng::scalar::Scalar;
use typeshare::typeshare;
pub use wallet::Wallet;
pub use wallet_rpc::{WalletRpc, WalletRpcProcess};

use crate::bitcoin;
use anyhow::Result;
use rand::{CryptoRng, RngCore};
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::fmt;
use std::ops::{Add, Mul, Sub};
use std::str::FromStr;

pub const BDX_OFFSET: u64 = 1_000_000_000_000;

#[derive(Serialize, Deserialize)]
#[serde(remote = "Network")]
#[allow(non_camel_case_types)]
pub enum network {
    Mainnet,
    Stagenet,
    Testnet,
}

pub fn private_key_from_secp256k1_scalar(scalar: bitcoin::Scalar) -> PrivateKey {
    let mut bytes = scalar.to_bytes();

    // we must reverse the bytes because a secp256k1 scalar is big endian, whereas a
    // ed25519 scalar is little endian
    bytes.reverse();

    PrivateKey::from_scalar(Scalar::from_bytes_mod_order(bytes))
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PrivateViewKey(#[serde(with = "beldex_private_key")] PrivateKey);

impl fmt::Display for PrivateViewKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Delegate to the Display implementation of PrivateKey
        write!(f, "{}", self.0)
    }
}

impl PrivateViewKey {
    pub fn new_random<R: RngCore + CryptoRng>(rng: &mut R) -> Self {
        let scalar = Scalar::random(rng);
        let private_key = PrivateKey::from_scalar(scalar);

        Self(private_key)
    }

    pub fn public(&self) -> PublicViewKey {
        PublicViewKey(PublicKey::from_private_key(&self.0))
    }
}

impl Add for PrivateViewKey {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl From<PrivateViewKey> for PrivateKey {
    fn from(from: PrivateViewKey) -> Self {
        from.0
    }
}

impl From<PublicViewKey> for PublicKey {
    fn from(from: PublicViewKey) -> Self {
        from.0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PublicViewKey(PublicKey);

#[derive(Debug, Copy, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd)]
#[typeshare(serialized_as = "number")]
pub struct Amount(u64);

// Median tx fees on Beldex. BDX 0.01 * 2 (to be on the safe side)
pub const BELDEX_FEE: Amount = Amount::from_atomic(20_000_000);

impl Amount {
    pub const ZERO: Self = Self(0);
    pub const ONE_BDX: Self = Self(BDX_OFFSET);
    /// Create an [Amount] with atomic unit precision and the given number of
    /// atomic units.
    ///
    /// An atomic unit is equal to 1e-9 BDX.
    pub const fn from_atomic(amount: u64) -> Self {
        Amount(amount)
    }

    /// Return Beldex Amount as atomic units.
    pub fn as_atomic(&self) -> u64 {
        self.0
    }

    /// Calculate the maximum amount of Bitcoin that can be bought at a given
    /// asking price for this amount of Beldex including the median fee.
    pub fn max_bitcoin_for_price(&self, ask_price: bitcoin::Amount) -> Option<bitcoin::Amount> {
        let atomic_minus_fee = self.as_atomic().saturating_sub(BELDEX_FEE.as_atomic());

        if atomic_minus_fee == 0 {
            return Some(bitcoin::Amount::ZERO);
        }

        // safely convert the BTC/BDX rate to sat/atomic
        let ask_sats = Decimal::from(ask_price.to_sat());
        let atomic_per_bdx = Decimal::from(BDX_OFFSET);
        let ask_sats_per_atomic = ask_sats / atomic_per_bdx;

        let atomic = Decimal::from(atomic_minus_fee);
        let max_sats = atomic.checked_mul(ask_sats_per_atomic)?;
        let satoshi = max_sats.to_u64()?;

        Some(bitcoin::Amount::from_sat(satoshi))
    }

    pub fn from_beldex(amount: f64) -> Result<Self> {
        let decimal = Decimal::try_from(amount)?;
        Self::from_decimal(decimal)
    }

    pub fn parse_beldex(amount: &str) -> Result<Self> {
        let decimal = Decimal::from_str(amount)?;
        Self::from_decimal(decimal)
    }

    pub fn as_atomic_decimal(&self) -> Decimal {
        Decimal::from(self.as_atomic())
    }

    fn from_decimal(amount: Decimal) -> Result<Self> {
        let atomics_dec =
            amount.mul(Decimal::from_u64(BDX_OFFSET).expect("constant to fit into u64"));
        let atomics = atomics_dec
            .to_u64()
            .ok_or_else(|| OverflowError(amount.to_string()))?;
        Ok(Amount(atomics))
    }
}

impl Add for Amount {
    type Output = Amount;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Sub for Amount {
    type Output = Amount;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl Mul<u64> for Amount {
    type Output = Amount;

    fn mul(self, rhs: u64) -> Self::Output {
        Self(self.0 * rhs)
    }
}

impl From<Amount> for u64 {
    fn from(from: Amount) -> u64 {
        from.0
    }
}

impl fmt::Display for Amount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut decimal = Decimal::from(self.0);
        decimal
            .set_scale(12)
            .expect("12 is smaller than max precision of 28");
        write!(f, "{} BDX", decimal)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransferProof {
    tx_hash: TxHash,
    #[serde(with = "beldex_private_key")]
    tx_key: PrivateKey,
}

impl TransferProof {
    pub fn new(tx_hash: TxHash, tx_key: PrivateKey) -> Self {
        Self { tx_hash, tx_key }
    }
    pub fn tx_hash(&self) -> TxHash {
        self.tx_hash.clone()
    }
    pub fn tx_key(&self) -> PrivateKey {
        self.tx_key
    }
}

// TODO: add constructor/ change String to fixed length byte array
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TxHash(pub String);

impl From<TxHash> for String {
    fn from(from: TxHash) -> Self {
        from.0
    }
}

impl fmt::Display for TxHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("expected {expected}, got {actual}")]
pub struct InsufficientFunds {
    pub expected: Amount,
    pub actual: Amount,
}

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
#[error("Overflow, cannot convert {0} to u64")]
pub struct OverflowError(pub String);

pub mod beldex_private_key {
    use beldex::consensus::{Decodable, Encodable};
    use beldex::PrivateKey;
    use serde::de::Visitor;
    use serde::ser::Error;
    use serde::{de, Deserializer, Serializer};
    use std::fmt;
    use std::io::Cursor;

    struct BytesVisitor;

    impl<'de> Visitor<'de> for BytesVisitor {
        type Value = PrivateKey;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(formatter, "a byte array representing a Beldex private key")
        }

        fn visit_bytes<E>(self, s: &[u8]) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            let mut s = s;
            PrivateKey::consensus_decode(&mut s).map_err(|err| E::custom(format!("{:?}", err)))
        }

        fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            let bytes = hex::decode(s).map_err(|err| E::custom(format!("{:?}", err)))?;
            PrivateKey::consensus_decode(&mut bytes.as_slice())
                .map_err(|err| E::custom(format!("{:?}", err)))
        }
    }

    pub fn serialize<S>(x: &PrivateKey, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut bytes = Cursor::new(vec![]);
        x.consensus_encode(&mut bytes)
            .map_err(|err| S::Error::custom(format!("{:?}", err)))?;
        if s.is_human_readable() {
            s.serialize_str(&hex::encode(bytes.into_inner()))
        } else {
            s.serialize_bytes(bytes.into_inner().as_ref())
        }
    }

    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<PrivateKey, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
    {
        let key = {
            if deserializer.is_human_readable() {
                deserializer.deserialize_string(BytesVisitor)?
            } else {
                deserializer.deserialize_bytes(BytesVisitor)?
            }
        };
        Ok(key)
    }
}

pub mod beldex_amount {
    use crate::beldex::Amount;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(x: &Amount, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        s.serialize_u64(x.as_atomic())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Amount, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
    {
        let atomics = u64::deserialize(deserializer)?;
        let amount = Amount::from_atomic(atomics);

        Ok(amount)
    }
}

pub mod beldex_address {
    use anyhow::{bail, Context, Result};
    use std::str::FromStr;

    #[derive(thiserror::Error, Debug, Clone, Copy, PartialEq)]
    #[error("Invalid Beldex address provided, expected address on network {expected:?} but address provided is on {actual:?}")]
    pub struct BeldexAddressNetworkMismatch {
        pub expected: beldex::Network,
        pub actual: beldex::Network,
    }

    pub fn parse(s: &str) -> Result<beldex::Address> {
        beldex::Address::from_str(s).with_context(|| {
            format!(
                "Failed to parse {} as a Beldex address, please make sure it is a valid address",
                s
            )
        })
    }

    pub fn validate(
        address: beldex::Address,
        expected_network: beldex::Network,
    ) -> Result<beldex::Address> {
        if address.network != expected_network {
            bail!(BeldexAddressNetworkMismatch {
                expected: expected_network,
                actual: address.network,
            });
        }
        Ok(address)
    }

    pub fn validate_is_testnet(
        address: beldex::Address,
        is_testnet: bool,
    ) -> Result<beldex::Address> {
        let expected_network = if is_testnet {
            beldex::Network::Testnet
        } else {
            beldex::Network::Mainnet
        };
        validate(address, expected_network)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_beldex_min() {
        let min_pics = 1;
        let amount = Amount::from_atomic(min_pics);
        let beldex = amount.to_string();
        assert_eq!("0.000000000001 BDX", beldex);
    }

    #[test]
    fn display_beldex_one() {
        let min_pics = 1000000000000;
        let amount = Amount::from_atomic(min_pics);
        let beldex = amount.to_string();
        assert_eq!("1.000000000000 BDX", beldex);
    }

    #[test]
    fn display_beldex_max() {
        let max_pics = 18_446_744_073_709_551_615;
        let amount = Amount::from_atomic(max_pics);
        let beldex = amount.to_string();
        assert_eq!("18446744.073709551615 BDX", beldex);
    }

    #[test]
    fn parse_beldex_min() {
        let beldex_min = "0.000000000001";
        let amount = Amount::parse_beldex(beldex_min).unwrap();
        let pics = amount.0;
        assert_eq!(1, pics);
    }

    #[test]
    fn parse_beldex() {
        let beldex = "123";
        let amount = Amount::parse_beldex(beldex).unwrap();
        let pics = amount.0;
        assert_eq!(123000000000000, pics);
    }

    #[test]
    fn parse_beldex_max() {
        let beldex = "18446744.073709551615";
        let amount = Amount::parse_beldex(beldex).unwrap();
        let pics = amount.0;
        assert_eq!(18446744073709551615, pics);
    }

    #[test]
    fn parse_beldex_overflows() {
        let overflow_pics = "18446744.073709551616";
        let error = Amount::parse_beldex(overflow_pics).unwrap_err();
        assert_eq!(
            error.downcast_ref::<OverflowError>().unwrap(),
            &OverflowError(overflow_pics.to_owned())
        );
    }

    #[test]
    fn max_bitcoin_to_trade() {
        // sanity check: if the asking price is 1 BTC / 1 BDX
        // and we have μ BDX + fee
        // then max BTC we can buy is μ
        let ask = bitcoin::Amount::from_btc(1.0).unwrap();

        let bdx = Amount::parse_beldex("1.0").unwrap() + BELDEX_FEE;
        let btc = bdx.max_bitcoin_for_price(ask).unwrap();

        assert_eq!(btc, bitcoin::Amount::from_btc(1.0).unwrap());

        let bdx = Amount::parse_beldex("0.5").unwrap() + BELDEX_FEE;
        let btc = bdx.max_bitcoin_for_price(ask).unwrap();

        assert_eq!(btc, bitcoin::Amount::from_btc(0.5).unwrap());

        let bdx = Amount::parse_beldex("2.5").unwrap() + BELDEX_FEE;
        let btc = bdx.max_bitcoin_for_price(ask).unwrap();

        assert_eq!(btc, bitcoin::Amount::from_btc(2.5).unwrap());

        let bdx = Amount::parse_beldex("420").unwrap() + BELDEX_FEE;
        let btc = bdx.max_bitcoin_for_price(ask).unwrap();

        assert_eq!(btc, bitcoin::Amount::from_btc(420.0).unwrap());

        let bdx = Amount::parse_beldex("0.00001").unwrap() + BELDEX_FEE;
        let btc = bdx.max_bitcoin_for_price(ask).unwrap();

        assert_eq!(btc, bitcoin::Amount::from_btc(0.00001).unwrap());

        // other ask prices

        let ask = bitcoin::Amount::from_btc(0.5).unwrap();
        let bdx = Amount::parse_beldex("2").unwrap() + BELDEX_FEE;
        let btc = bdx.max_bitcoin_for_price(ask).unwrap();

        assert_eq!(btc, bitcoin::Amount::from_btc(1.0).unwrap());

        let ask = bitcoin::Amount::from_btc(2.0).unwrap();
        let bdx = Amount::parse_beldex("1").unwrap() + BELDEX_FEE;
        let btc = bdx.max_bitcoin_for_price(ask).unwrap();

        assert_eq!(btc, bitcoin::Amount::from_btc(2.0).unwrap());

        let ask = bitcoin::Amount::from_sat(382_900);
        let bdx = Amount::parse_beldex("10").unwrap();
        let btc = bdx.max_bitcoin_for_price(ask).unwrap();

        assert_eq!(btc, bitcoin::Amount::from_sat(3_828_993));

        // example from https://github.com/comit-network/bdx-btc-swap/issues/1084
        // with rate from kraken at that time
        let ask = bitcoin::Amount::from_sat(685_800);
        let bdx = Amount::parse_beldex("0.826286435921").unwrap();
        let btc = bdx.max_bitcoin_for_price(ask).unwrap();

        assert_eq!(btc, bitcoin::Amount::from_sat(566_656));
    }

    #[test]
    fn max_bitcoin_to_trade_overflow() {
        let bdx = Amount::from_beldex(30.0).unwrap();
        let ask = bitcoin::Amount::from_sat(728_688);
        let btc = bdx.max_bitcoin_for_price(ask).unwrap();

        assert_eq!(bitcoin::Amount::from_sat(21_860_628), btc);

        let bdx = Amount::from_atomic(u64::MAX);
        let ask = bitcoin::Amount::from_sat(u64::MAX);
        let btc = bdx.max_bitcoin_for_price(ask);

        assert!(btc.is_none());
    }

    #[test]
    fn geting_max_bitcoin_to_trade_with_balance_smaller_than_locking_fee() {
        let ask = bitcoin::Amount::from_sat(382_900);
        let bdx = Amount::parse_beldex("0.00001").unwrap();
        let btc = bdx.max_bitcoin_for_price(ask).unwrap();

        assert_eq!(bitcoin::Amount::ZERO, btc);
    }

    use rand::rngs::OsRng;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub struct BeldexPrivateKey(#[serde(with = "beldex_private_key")] crate::beldex::PrivateKey);

    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub struct BeldexAmount(#[serde(with = "beldex_amount")] crate::beldex::Amount);

    #[test]
    fn serde_beldex_private_key_json() {
        let key = BeldexPrivateKey(beldex::PrivateKey::from_scalar(
            crate::beldex::Scalar::random(&mut OsRng),
        ));
        let encoded = serde_json::to_vec(&key).unwrap();
        let decoded: BeldexPrivateKey = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(key, decoded);
    }

    #[test]
    fn serde_beldex_private_key_cbor() {
        let key = BeldexPrivateKey(beldex::PrivateKey::from_scalar(
            crate::beldex::Scalar::random(&mut OsRng),
        ));
        let encoded = serde_cbor::to_vec(&key).unwrap();
        let decoded: BeldexPrivateKey = serde_cbor::from_slice(&encoded).unwrap();
        assert_eq!(key, decoded);
    }

    #[test]
    fn serde_beldex_amount() {
        let amount = BeldexAmount(crate::beldex::Amount::from_atomic(1000));
        let encoded = serde_cbor::to_vec(&amount).unwrap();
        let decoded: BeldexAmount = serde_cbor::from_slice(&encoded).unwrap();
        assert_eq!(amount, decoded);
    }
}
