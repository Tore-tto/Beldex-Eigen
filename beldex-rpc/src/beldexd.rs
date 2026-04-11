use anyhow::{Context, Result};
use beldex::cryptonote::hash::Hash;
use beldex::util::ringct;
use beldex::PublicKey;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize, Serializer};

#[async_trait::async_trait]
pub trait BeldexdRpc<E: std::error::Error + Send + Sync + 'static> {
    async fn generateblocks(
        &self,
        amount_of_blocks: u32,
        wallet_address: String,
    ) -> Result<GenerateBlocks, jsonrpc_client::Error<E>>;
    async fn get_block_header_by_height(
        &self,
        height: u32,
    ) -> Result<BlockHeader, jsonrpc_client::Error<E>>;
    async fn get_block_count(&self) -> Result<BlockCount, jsonrpc_client::Error<E>>;
    async fn get_version(&self) -> Result<Version, jsonrpc_client::Error<E>>;
    async fn get_block(&self, height: u32) -> Result<GetBlockResponse, jsonrpc_client::Error<E>>;
}

#[derive(Debug, Clone)]
pub struct Client {
    inner: reqwest::Client,
    base_url: reqwest::Url,
    get_o_indexes_bin_url: reqwest::Url,
    get_outs_bin_url: reqwest::Url,
}

impl Client {
    /// New local host beldexd RPC client.
    pub fn localhost(port: u16) -> Result<Self> {
        Self::new("127.0.0.1".to_owned(), port)
    }

    fn new(host: String, port: u16) -> Result<Self> {
        Ok(Self {
            inner: reqwest::ClientBuilder::new()
                .connection_verbose(true)
                .build()?,
            base_url: format!("http://{}:{}/json_rpc", host, port)
                .parse()
                .context("url is well formed")?,
            get_o_indexes_bin_url: format!("http://{}:{}/get_o_indexes.bin", host, port)
                .parse()
                .context("url is well formed")?,
            get_outs_bin_url: format!("http://{}:{}/get_outs.bin", host, port)
                .parse()
                .context("url is well formed")?,
        })
    }

    async fn call<P: Serialize, R: DeserializeOwned>(
        &self,
        method: &str,
        params: P,
    ) -> Result<R, jsonrpc_client::Error<reqwest::Error>> {
        let response = self
            .send_request(method, params)
            .await
            .map_err(jsonrpc_client::Error::Client)?;

        if let Some(error) = response.error {
            return Err(jsonrpc_client::Error::JsonRpc(jsonrpc_client::JsonRpcError {
                code: error.code,
                message: error.message,
                data: error.data,
            }));
        }

        Ok(response.result.expect("result or error must be present"))
    }

    async fn send_request<P: Serialize, R: DeserializeOwned>(
        &self,
        method: &str,
        params: P,
    ) -> Result<RpcResponse<R>, reqwest::Error> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "0", // Force string ID to avoid parsing errors in some Beldex/Monero versions
            "method": method,
            "params": params,
        });

        self.inner
            .post(self.base_url.clone())
            .json(&request)
            .send()
            .await?
            .json()
            .await
    }

    pub async fn get_o_indexes(&self, txid: Hash) -> Result<GetOIndexesResponse> {
        self.binary_request(
            self.get_o_indexes_bin_url.clone(),
            GetOIndexesPayload { txid },
        )
        .await
    }

    pub async fn get_outs(&self, outputs: Vec<GetOutputsOut>) -> Result<GetOutsResponse> {
        self.binary_request(self.get_outs_bin_url.clone(), GetOutsPayload { outputs })
            .await
    }

    async fn binary_request<Req, Res>(&self, url: reqwest::Url, request: Req) -> Result<Res>
    where
        Req: Serialize,
        Res: DeserializeOwned,
    {
        let response = self
            .inner
            .post(url)
            .body(beldex_epee_bin_serde::to_bytes(&request)?)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Request failed with status code {}", response.status())
        }

        let body = response.bytes().await?;

        Ok(beldex_epee_bin_serde::from_bytes(body)?)
    }
}

#[async_trait::async_trait]
impl BeldexdRpc<reqwest::Error> for Client {
    async fn generateblocks(
        &self,
        amount_of_blocks: u32,
        wallet_address: String,
    ) -> Result<GenerateBlocks, jsonrpc_client::Error<reqwest::Error>> {
        self.call(
            "generateblocks",
            serde_json::json!({
                "amount_of_blocks": amount_of_blocks,
                "wallet_address": wallet_address
            }),
        )
        .await
    }

    async fn get_block_header_by_height(
        &self,
        height: u32,
    ) -> Result<BlockHeader, jsonrpc_client::Error<reqwest::Error>> {
        self.call(
            "get_block_header_by_height",
            serde_json::json!({ "height": height }),
        )
        .await
    }

    async fn get_block_count(&self) -> Result<BlockCount, jsonrpc_client::Error<reqwest::Error>> {
        self.call("get_block_count", serde_json::json!({})).await
    }

    async fn get_version(&self) -> Result<Version, jsonrpc_client::Error<reqwest::Error>> {
        self.call("get_version", serde_json::json!({})).await
    }

    async fn get_block(&self, height: u32) -> Result<GetBlockResponse, jsonrpc_client::Error<reqwest::Error>> {
        self.call("get_block", serde_json::json!({ "height": height })).await
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

#[derive(Deserialize, Debug)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct GenerateBlocks {
    pub blocks: Vec<String>,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct BlockCount {
    pub count: u32,
}

#[derive(Debug, Copy, Clone, Deserialize)]
pub struct Version {
    pub version: u32,
}

// We should be able to use beldex-rs for this but it does not include all
// the fields.
#[derive(Clone, Debug, Deserialize)]
pub struct BlockHeader {
    pub block_size: u32,
    pub depth: u32,
    pub difficulty: u32,
    pub hash: String,
    pub height: u32,
    pub major_version: u32,
    pub minor_version: u32,
    pub nonce: u32,
    pub num_txes: u32,
    pub orphan_status: bool,
    pub prev_hash: String,
    pub reward: u64,
    pub timestamp: u32,
}

#[derive(Debug, Deserialize)]
pub struct GetBlockResponse {
    #[serde(with = "beldex_serde_hex_block")]
    pub blob: beldex::Block,
}

#[derive(Debug, Deserialize)]
pub struct GetIndexesResponse {
    pub o_indexes: Vec<u32>,
}

#[derive(Clone, Debug, Serialize)]
struct GetOIndexesPayload {
    #[serde(with = "byte_array")]
    txid: Hash,
}

#[derive(Clone, Debug, Serialize)]
struct GetOutsPayload {
    outputs: Vec<GetOutputsOut>,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct GetOutputsOut {
    pub amount: u64,
    pub index: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct GetOutsResponse {
    #[serde(flatten)]
    pub base: BaseResponse,
    pub outs: Vec<OutKey>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub struct OutKey {
    pub height: u64,
    #[serde(with = "byte_array")]
    pub key: PublicKey,
    #[serde(with = "byte_array")]
    pub mask: ringct::Key,
    #[serde(with = "byte_array")]
    pub txid: Hash,
    pub unlocked: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct BaseResponse {
    pub credits: u64,
    pub status: Status,
    pub top_hash: String,
    pub untrusted: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct GetOIndexesResponse {
    #[serde(flatten)]
    pub base: BaseResponse,
    #[serde(default)]
    pub o_indexes: Vec<u64>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub enum Status {
    #[serde(rename = "OK")]
    Ok,
    #[serde(rename = "Failed")]
    Failed,
}

mod beldex_serde_hex_block {
    use super::*;
    use beldex::consensus::Decodable;
    use serde::de::Error;
    use serde::{Deserialize, Deserializer};
    use std::io::Cursor;

    pub fn deserialize<'de, D>(deserializer: D) -> Result<beldex::Block, D::Error>
    where
        D: Deserializer<'de>,
    {
        let hex = String::deserialize(deserializer)?;

        let bytes = hex::decode(hex).map_err(D::Error::custom)?;
        let mut cursor = Cursor::new(bytes);

        let block = beldex::Block::consensus_decode(&mut cursor).map_err(D::Error::custom)?;

        Ok(block)
    }
}

mod byte_array {
    use super::*;
    use serde::de::Error;
    use serde::Deserializer;
    use std::convert::TryFrom;
    use std::fmt;
    use std::marker::PhantomData;

    pub fn serialize<S, B>(bytes: B, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        B: AsRef<[u8]>,
    {
        serializer.serialize_bytes(bytes.as_ref())
    }

    pub fn deserialize<'de, D, B, const N: usize>(deserializer: D) -> Result<B, D::Error>
    where
        D: Deserializer<'de>,
        B: TryFrom<[u8; N]>,
    {
        struct Visitor<T, const N: usize> {
            phantom: PhantomData<(T, [u8; N])>,
        }

        impl<'de, T, const N: usize> serde::de::Visitor<'de> for Visitor<T, N>
        where
            T: TryFrom<[u8; N]>,
        {
            type Value = T;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "a byte buffer")
            }

            fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Self::Value, E>
            where
                E: Error,
            {
                let bytes = <[u8; N]>::try_from(v).map_err(|_| {
                    E::custom(format!("Failed to construct [u8; {}] from buffer", N))
                })?;
                let result = T::try_from(bytes)
                    .map_err(|_| E::custom(format!("Failed to construct T from [u8; {}]", N)))?;

                Ok(result)
            }
        }

        deserializer.deserialize_byte_buf(Visitor {
            phantom: PhantomData,
        })
    }
}
