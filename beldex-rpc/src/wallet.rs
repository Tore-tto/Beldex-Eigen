use std::fmt;

use anyhow::{Context, Result};
use rust_decimal::Decimal;
use serde::de::Error;
use serde::{Deserialize, Deserializer, Serialize};

use serde::de::DeserializeOwned;

#[async_trait::async_trait]
pub trait BeldexWalletRpc<E: std::error::Error + Send + Sync + 'static> {
    async fn get_address(&self, account_index: u32)
        -> Result<GetAddress, jsonrpc_client::Error<E>>;
    async fn get_balance(&self, account_index: u32)
        -> Result<GetBalance, jsonrpc_client::Error<E>>;
    async fn create_account(
        &self,
        label: String,
    ) -> Result<CreateAccount, jsonrpc_client::Error<E>>;
    async fn get_accounts(&self, tag: String) -> Result<GetAccounts, jsonrpc_client::Error<E>>;
    async fn open_wallet(&self, filename: String)
        -> Result<WalletOpened, jsonrpc_client::Error<E>>;
    async fn close_wallet(&self) -> Result<WalletClosed, jsonrpc_client::Error<E>>;
    async fn create_wallet(
        &self,
        filename: String,
        language: String,
    ) -> Result<WalletCreated, jsonrpc_client::Error<E>>;
    async fn transfer(
        &self,
        account_index: u32,
        destinations: Vec<Destination>,
        get_tx_key: bool,
    ) -> Result<Transfer, jsonrpc_client::Error<E>>;
    async fn get_height(&self) -> Result<BlockHeight, jsonrpc_client::Error<E>>;
    async fn check_tx_key(
        &self,
        txid: String,
        tx_key: String,
        address: String,
    ) -> Result<CheckTxKey, jsonrpc_client::Error<E>>;
    #[allow(clippy::too_many_arguments)]
    async fn generate_from_keys(
        &self,
        filename: String,
        address: String,
        spendkey: String,
        viewkey: String,
        restore_height: u32,
        password: String,
        autosave_current: bool,
    ) -> Result<GenerateFromKeys, jsonrpc_client::Error<E>>;
    async fn refresh(&self) -> Result<Refreshed, jsonrpc_client::Error<E>>;
    async fn sweep_all(&self, address: String) -> Result<SweepAll, jsonrpc_client::Error<E>>;
    async fn get_version(&self) -> Result<Version, jsonrpc_client::Error<E>>;
}

#[derive(Debug, Clone)]
pub struct Client {
    inner: reqwest::Client,
    base_url: reqwest::Url,
}

impl Client {
    /// Constructs a beldex-wallet-rpc client with localhost endpoint.
    pub fn localhost(port: u16) -> Result<Self> {
        Client::new(
            format!("http://127.0.0.1:{}/json_rpc", port)
                .parse()
                .context("url is well formed")?,
        )
    }

    /// Constructs a beldex-wallet-rpc client with `url` endpoint.
    pub fn new(url: reqwest::Url) -> Result<Self> {
        Ok(Self {
            inner: reqwest::ClientBuilder::new()
                .connection_verbose(true)
                .build()?,
            base_url: url,
        })
    }

    /// Transfers `amount` beldex from `account_index` to `address`.
    pub async fn transfer_single(
        &self,
        account_index: u32,
        amount: u64,
        address: &str,
    ) -> Result<Transfer> {
        let dest = vec![Destination {
            amount,
            address: address.to_owned(),
        }];

        Ok(self.transfer(account_index, dest, true).await?)
    }

    async fn call<P: Serialize, R: DeserializeOwned>(
        &self,
        method: &str,
        params: P,
    ) -> Result<R, jsonrpc_client::Error<reqwest::Error>> {
        let text = self
            .send_request(method, params)
            .await
            .map_err(jsonrpc_client::Error::Client)?;

        let response: RpcResponse<R> = serde_json::from_str(&text).map_err(|e| {
            jsonrpc_client::Error::JsonRpc(jsonrpc_client::JsonRpcError {
                code: -32700,
                message: format!("Parse error: {}", e),
                data: None,
            })
        })?;

        response.into_result().map_err(|error| {
            jsonrpc_client::Error::JsonRpc(jsonrpc_client::JsonRpcError {
                code: error.code,
                message: error.message,
                data: error.data,
            })
        })
    }

    async fn send_request<P: Serialize>(
        &self,
        method: &str,
        params: P,
    ) -> Result<String, reqwest::Error> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "1",
            "method": method,
            "params": params,
        });

        self.inner
            .post(self.base_url.clone())
            .json(&request)
            .send()
            .await?
            .text()
            .await
    }
}

#[derive(Deserialize, Debug)]
struct RpcResponse<R> {
    #[allow(dead_code)]
    pub jsonrpc: String,
    pub result: Option<R>,
    pub error: Option<RpcError>,
    #[allow(dead_code)]
    pub id: serde_json::Value,
}

impl<R> RpcResponse<R> {
    pub fn into_result(self) -> Result<R, RpcError> {
        match (self.result, self.error) {
            (Some(result), None) => Ok(result),
            (None, Some(error)) => Err(error),
            (Some(_), Some(error)) => Err(error),
            (None, None) => Err(RpcError {
                code: -32603,
                message: "Internal JSON-RPC error: response has neither result nor error"
                    .to_string(),
                data: None,
            }),
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

#[async_trait::async_trait]
impl BeldexWalletRpc<reqwest::Error> for Client {
    async fn get_address(
        &self,
        account_index: u32,
    ) -> Result<GetAddress, jsonrpc_client::Error<reqwest::Error>> {
        self.call(
            "get_address",
            serde_json::json!({ "account_index": account_index }),
        )
        .await
    }

    async fn get_balance(
        &self,
        account_index: u32,
    ) -> Result<GetBalance, jsonrpc_client::Error<reqwest::Error>> {
        self.call(
            "get_balance",
            serde_json::json!({ "account_index": account_index }),
        )
        .await
    }

    async fn create_account(
        &self,
        label: String,
    ) -> Result<CreateAccount, jsonrpc_client::Error<reqwest::Error>> {
        self.call("create_account", serde_json::json!({ "label": label }))
            .await
    }

    async fn get_accounts(
        &self,
        tag: String,
    ) -> Result<GetAccounts, jsonrpc_client::Error<reqwest::Error>> {
        self.call("get_accounts", serde_json::json!({ "tag": tag }))
            .await
    }

    async fn open_wallet(
        &self,
        filename: String,
    ) -> Result<WalletOpened, jsonrpc_client::Error<reqwest::Error>> {
        self.call("open_wallet", serde_json::json!({ "filename": filename }))
            .await
    }

    async fn close_wallet(&self) -> Result<WalletClosed, jsonrpc_client::Error<reqwest::Error>> {
        self.call("close_wallet", serde_json::json!({})).await
    }

    async fn create_wallet(
        &self,
        filename: String,
        language: String,
    ) -> Result<WalletCreated, jsonrpc_client::Error<reqwest::Error>> {
        self.call(
            "create_wallet",
            serde_json::json!({ "filename": filename, "language": language }),
        )
        .await
    }

    async fn transfer(
        &self,
        account_index: u32,
        destinations: Vec<Destination>,
        get_tx_key: bool,
    ) -> Result<Transfer, jsonrpc_client::Error<reqwest::Error>> {
        self.call(
            "transfer",
            serde_json::json!({
                "account_index": account_index,
                "destinations": destinations,
                "get_tx_key": get_tx_key,
            }),
        )
        .await
    }

    async fn get_height(&self) -> Result<BlockHeight, jsonrpc_client::Error<reqwest::Error>> {
        self.call("get_height", serde_json::json!({})).await
    }

    async fn check_tx_key(
        &self,
        txid: String,
        tx_key: String,
        address: String,
    ) -> Result<CheckTxKey, jsonrpc_client::Error<reqwest::Error>> {
        self.call(
            "check_tx_key",
            serde_json::json!({ "txid": txid, "tx_key": tx_key, "address": address }),
        )
        .await
    }

    async fn generate_from_keys(
        &self,
        filename: String,
        address: String,
        spendkey: String,
        viewkey: String,
        restore_height: u32,
        password: String,
        autosave_current: bool,
    ) -> Result<GenerateFromKeys, jsonrpc_client::Error<reqwest::Error>> {
        self.call("generate_from_keys", serde_json::json!({ "filename": filename, "address": address, "spendkey": spendkey, "viewkey": viewkey, "restore_height": restore_height, "password": password, "autosave_current": autosave_current })).await
    }

    async fn refresh(&self) -> Result<Refreshed, jsonrpc_client::Error<reqwest::Error>> {
        self.call("refresh", serde_json::json!({})).await
    }

    async fn sweep_all(
        &self,
        address: String,
    ) -> Result<SweepAll, jsonrpc_client::Error<reqwest::Error>> {
        self.call("sweep_all", serde_json::json!({ "address": address }))
            .await
    }

    async fn get_version(&self) -> Result<Version, jsonrpc_client::Error<reqwest::Error>> {
        self.call("get_version", serde_json::json!({})).await
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct GetAddress {
    pub address: String,
}

#[derive(Deserialize, Debug, Clone, Copy)]
pub struct GetBalance {
    pub balance: u64,
    pub unlocked_balance: u64,
    pub multisig_import_needed: bool,
    pub blocks_to_unlock: u32,
    pub time_to_unlock: u32,
}

impl fmt::Display for GetBalance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut total = Decimal::from(self.balance);
        total
            .set_scale(9)
            .expect("12 is smaller than max precision of 28");

        let mut unlocked = Decimal::from(self.unlocked_balance);
        unlocked
            .set_scale(9)
            .expect("12 is smaller than max precision of 28");

        write!(
            f,
            "total balance: {}, unlocked balance: {}",
            total, unlocked
        )
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct CreateAccount {
    pub account_index: u32,
    pub address: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct GetAccounts {
    pub subaddress_accounts: Vec<SubAddressAccount>,
    pub total_balance: u64,
    pub total_unlocked_balance: u64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SubAddressAccount {
    pub account_index: u32,
    pub balance: u32,
    pub base_address: String,
    pub label: String,
    pub tag: String,
    pub unlocked_balance: u64,
}

#[derive(Serialize, Debug, Clone)]
pub struct Destination {
    pub amount: u64,
    pub address: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Transfer {
    pub amount: u64,
    pub fee: u64,
    pub multisig_txset: String,
    pub tx_blob: String,
    pub tx_hash: String,
    #[serde(deserialize_with = "opt_key_from_blank")]
    pub tx_key: Option<beldex::PrivateKey>,
    pub tx_metadata: String,
    pub unsigned_txset: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct BlockHeight {
    pub height: u32,
}

impl fmt::Display for BlockHeight {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.height)
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(from = "CheckTxKeyResponse")]
pub struct CheckTxKey {
    pub confirmations: u64,
    pub received: u64,
    pub in_pool: bool,
}

#[derive(Clone, Copy, Debug, Deserialize)]
struct CheckTxKeyResponse {
    pub confirmations: u64,
    pub received: u64,
    pub in_pool: bool,
}

impl From<CheckTxKeyResponse> for CheckTxKey {
    fn from(response: CheckTxKeyResponse) -> Self {
        // Due to a bug in beldexd that causes check_tx_key confirmations
        // to overflow we safeguard the confirmations to avoid unwanted
        // side effects.
        let confirmations = if response.confirmations > u64::MAX - 1000 {
            0
        } else {
            response.confirmations
        };

        CheckTxKey {
            confirmations,
            received: response.received,
            in_pool: response.in_pool,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct GenerateFromKeys {
    pub address: String,
    pub info: String,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct Refreshed {
    pub blocks_fetched: u32,
    pub received_money: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SweepAll {
    pub tx_hash_list: Vec<String>,
}

#[derive(Debug, Copy, Clone, Deserialize)]
pub struct Version {
    pub version: u32,
}

pub type WalletCreated = Empty;
pub type WalletClosed = Empty;
pub type WalletOpened = Empty;

/// Zero-sized struct to allow serde to deserialize an empty JSON object.
///
/// With `serde`, an empty JSON object (`{ }`) does not deserialize into Rust's
/// `()`. With the adoption of `jsonrpc_client`, we need to be explicit about
/// what the response of every RPC call is. Unfortunately, beldexd likes to
/// return empty objects instead of `null`s in certain cases. We use this struct
/// to all the "deserialization" to happily continue.
#[derive(Debug, Copy, Clone, Deserialize)]
pub struct Empty {}

fn opt_key_from_blank<'de, D>(deserializer: D) -> Result<Option<beldex::PrivateKey>, D::Error>
where
    D: Deserializer<'de>,
{
    let string = String::deserialize(deserializer)?;

    if string.is_empty() {
        return Ok(None);
    }

    Ok(Some(string.parse().map_err(D::Error::custom)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonrpc_client::Response;

    #[test]
    fn can_deserialize_sweep_all_response() {
        let response = r#"{
          "id": "0",
          "jsonrpc": "2.0",
          "result": {
            "amount_list": [29921410000],
            "fee_list": [78590000],
            "multisig_txset": "",
            "tx_hash_list": ["c1d8cfa87d445c1915a59d67be3e93ba8a29018640cf69b465f07b1840a8f8c8"],
            "unsigned_txset": "",
            "weight_list": [1448]
          }
        }"#;

        let _: Response<SweepAll> = serde_json::from_str(response).unwrap();
    }

    #[test]
    fn can_deserialize_create_wallet() {
        let response = r#"{
          "id": 0,
          "jsonrpc": "2.0",
          "result": {
          }
        }"#;

        let _: Response<WalletCreated> = serde_json::from_str(response).unwrap();
    }
}
