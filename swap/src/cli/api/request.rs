use super::tauri_bindings::TauriHandle;
use crate::bitcoin::{CancelTimelock, ExpiredTimelocks, PunishTimelock, TxLock};
use crate::cli::api::tauri_bindings::{TauriEmitter, TauriSwapProgressEvent};
use crate::cli::api::Context;
use crate::cli::{list_sellers as list_sellers_impl, EventLoop, SellerStatus};
use crate::common::get_logs;
use crate::libp2p_ext::MultiAddrExt;
use crate::network::quote::{BidQuote, ZeroQuoteReceived};
use crate::network::swarm;
use crate::protocol::bob::{BobState, Swap};
use crate::protocol::{bob, State};
use crate::{bitcoin, cli, beldex, rpc};
use ::bitcoin::Txid;
use anyhow::{bail, Context as AnyContext, Result};
use libp2p::core::Multiaddr;
use libp2p::PeerId;
use qrcode::render::unicode;
use qrcode::QrCode;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::cmp::min;
use std::convert::TryInto;
use std::future::Future;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tracing::Instrument;
use typeshare::typeshare;
use uuid::Uuid;

/// This trait is implemented by all types of request args that
/// the CLI can handle.
/// It provides a unified abstraction that can be useful for generics.
#[allow(async_fn_in_trait)]
pub trait Request {
    type Response: Serialize;
    async fn request(self, ctx: Arc<Context>) -> Result<Self::Response>;
}

// BuyBeldex
#[typeshare]
#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BuyBeldexArgs {
    #[typeshare(serialized_as = "string")]
    pub seller: Multiaddr,
    #[typeshare(serialized_as = "string")]
    pub bitcoin_change_address: bitcoin::Address,
    #[typeshare(serialized_as = "string")]
    pub beldex_receive_address: beldex::Address,
    #[typeshare(serialized_as = "number")]
    #[serde(default, with = "::bitcoin::util::amount::serde::as_sat::opt")]
    pub amount: Option<bitcoin::Amount>,
}

#[typeshare]
#[derive(Serialize, Deserialize, Debug)]
pub struct BuyBeldexResponse {
    #[typeshare(serialized_as = "string")]
    pub swap_id: Uuid,
    pub quote: BidQuote,
}

impl Request for BuyBeldexArgs {
    type Response = BuyBeldexResponse;

    async fn request(self, ctx: Arc<Context>) -> Result<Self::Response> {
        buy_beldex(self, ctx).await
    }
}

// ResumeSwap
#[typeshare]
#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ResumeSwapArgs {
    #[typeshare(serialized_as = "string")]
    pub swap_id: Uuid,
}

#[typeshare]
#[derive(Serialize, Deserialize, Debug)]
pub struct ResumeSwapResponse {
    pub result: String,
}

impl Request for ResumeSwapArgs {
    type Response = ResumeSwapResponse;

    async fn request(self, ctx: Arc<Context>) -> Result<Self::Response> {
        resume_swap(self, ctx).await
    }
}

// CancelAndRefund
#[typeshare]
#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CancelAndRefundArgs {
    #[typeshare(serialized_as = "string")]
    pub swap_id: Uuid,
}

impl Request for CancelAndRefundArgs {
    type Response = serde_json::Value;

    async fn request(self, ctx: Arc<Context>) -> Result<Self::Response> {
        cancel_and_refund(self, ctx).await
    }
}

// BeldexRecovery
#[typeshare]
#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BeldexRecoveryArgs {
    #[typeshare(serialized_as = "string")]
    pub swap_id: Uuid,
}

impl Request for BeldexRecoveryArgs {
    type Response = serde_json::Value;

    async fn request(self, ctx: Arc<Context>) -> Result<Self::Response> {
        beldex_recovery(self, ctx).await
    }
}

// WithdrawBtc
#[typeshare]
#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WithdrawBtcArgs {
    #[typeshare(serialized_as = "number")]
    #[serde(default, with = "::bitcoin::util::amount::serde::as_sat::opt")]
    pub amount: Option<bitcoin::Amount>,
    #[typeshare(serialized_as = "string")]
    pub address: bitcoin::Address,
}

#[typeshare]
#[derive(Serialize, Deserialize, Debug)]
pub struct WithdrawBtcResponse {
    #[typeshare(serialized_as = "number")]
    #[serde(with = "::bitcoin::util::amount::serde::as_sat")]
    pub amount: bitcoin::Amount,
    pub txid: String,
}

impl Request for WithdrawBtcArgs {
    type Response = WithdrawBtcResponse;

    async fn request(self, ctx: Arc<Context>) -> Result<Self::Response> {
        withdraw_btc(self, ctx).await
    }
}

// ListSellers
#[typeshare]
#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ListSellersArgs {
    #[typeshare(serialized_as = "string")]
    pub rendezvous_point: Multiaddr,
}

impl Request for ListSellersArgs {
    type Response = serde_json::Value;

    async fn request(self, ctx: Arc<Context>) -> Result<Self::Response> {
        list_sellers(self, ctx).await
    }
}

// StartDaemon
#[typeshare]
#[derive(Debug, Eq, PartialEq, Serialize, Deserialize, Default, Clone)]
pub struct StartDaemonArgs {
    #[typeshare(serialized_as = "string")]
    pub server_address: Option<SocketAddr>,
}

impl Request for StartDaemonArgs {
    type Response = serde_json::Value;

    async fn request(self, ctx: Arc<Context>) -> Result<Self::Response> {
        start_daemon(self, (*ctx).clone()).await
    }
}

// StopDaemon
#[derive(Debug, Eq, PartialEq, Serialize, Deserialize, Default, Clone)]
pub struct StopDaemonArgs;

impl Request for StopDaemonArgs {
    type Response = serde_json::Value;

    async fn request(self, ctx: Arc<Context>) -> Result<Self::Response> {
        stop_daemon(ctx).await
    }
}

// OpenDataDir
#[derive(Debug, Eq, PartialEq, Serialize, Deserialize, Default, Clone)]
pub struct OpenDataDirArgs;

impl Request for OpenDataDirArgs {
    type Response = serde_json::Value;

    async fn request(self, ctx: Arc<Context>) -> Result<Self::Response> {
        open_data_dir(ctx).await
    }
}

// GetSwapInfo
#[typeshare]
#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GetSwapInfoArgs {
    #[typeshare(serialized_as = "string")]
    pub swap_id: Uuid,
}

#[typeshare]
#[derive(Serialize)]
pub struct GetSwapInfoResponse {
    #[typeshare(serialized_as = "string")]
    pub swap_id: Uuid,
    pub seller: Seller,
    pub completed: bool,
    pub start_date: String,
    #[typeshare(serialized_as = "string")]
    pub state_name: String,
    #[typeshare(serialized_as = "number")]
    pub bdx_amount: beldex::Amount,
    #[typeshare(serialized_as = "number")]
    #[serde(with = "::bitcoin::util::amount::serde::as_sat")]
    pub btc_amount: bitcoin::Amount,
    #[typeshare(serialized_as = "string")]
    pub tx_lock_id: Txid,
    #[typeshare(serialized_as = "number")]
    #[serde(with = "::bitcoin::util::amount::serde::as_sat")]
    pub tx_cancel_fee: bitcoin::Amount,
    #[typeshare(serialized_as = "number")]
    #[serde(with = "::bitcoin::util::amount::serde::as_sat")]
    pub tx_refund_fee: bitcoin::Amount,
    #[typeshare(serialized_as = "number")]
    #[serde(with = "::bitcoin::util::amount::serde::as_sat")]
    pub tx_lock_fee: bitcoin::Amount,
    pub btc_refund_address: String,
    pub cancel_timelock: CancelTimelock,
    pub punish_timelock: PunishTimelock,
    pub timelock: Option<ExpiredTimelocks>,
}

impl Request for GetSwapInfoArgs {
    type Response = GetSwapInfoResponse;

    async fn request(self, ctx: Arc<Context>) -> Result<Self::Response> {
        get_swap_info(self, ctx).await
    }
}

// Balance
#[typeshare]
#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BalanceArgs {
    pub force_refresh: bool,
}

#[typeshare]
#[derive(Serialize, Deserialize, Debug)]
pub struct BalanceResponse {
    #[typeshare(serialized_as = "number")]
    #[serde(with = "::bitcoin::util::amount::serde::as_sat")]
    pub balance: bitcoin::Amount,
}

impl Request for BalanceArgs {
    type Response = BalanceResponse;

    async fn request(self, ctx: Arc<Context>) -> Result<Self::Response> {
        get_balance(self, ctx).await
    }
}

// GetHistory
#[typeshare]
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct GetHistoryArgs;

#[typeshare]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GetHistoryEntry {
    #[typeshare(serialized_as = "string")]
    swap_id: Uuid,
    state: String,
}

#[typeshare]
#[derive(Serialize, Deserialize, Debug)]
pub struct GetHistoryResponse {
    pub swaps: Vec<GetHistoryEntry>,
}

impl Request for GetHistoryArgs {
    type Response = GetHistoryResponse;

    async fn request(self, ctx: Arc<Context>) -> Result<Self::Response> {
        get_history(ctx).await
    }
}

// Additional structs
#[typeshare]
#[derive(Serialize, Deserialize, Debug)]
pub struct Seller {
    #[typeshare(serialized_as = "string")]
    pub peer_id: PeerId,
    pub addresses: Vec<String>,
}

// Suspend current swap
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct SuspendCurrentSwapArgs;

#[typeshare]
#[derive(Serialize, Deserialize, Debug)]
pub struct SuspendCurrentSwapResponse {
    #[typeshare(serialized_as = "string")]
    pub swap_id: Uuid,
}

impl Request for SuspendCurrentSwapArgs {
    type Response = SuspendCurrentSwapResponse;

    async fn request(self, ctx: Arc<Context>) -> Result<Self::Response> {
        suspend_current_swap(ctx).await
    }
}

pub struct GetCurrentSwapArgs;

impl Request for GetCurrentSwapArgs {
    type Response = serde_json::Value;

    async fn request(self, ctx: Arc<Context>) -> Result<Self::Response> {
        get_current_swap(ctx).await
    }
}

pub struct GetConfig;

impl Request for GetConfig {
    type Response = serde_json::Value;

    async fn request(self, ctx: Arc<Context>) -> Result<Self::Response> {
        get_config(ctx).await
    }
}

pub struct ExportBitcoinWalletArgs;

impl Request for ExportBitcoinWalletArgs {
    type Response = serde_json::Value;

    async fn request(self, ctx: Arc<Context>) -> Result<Self::Response> {
        export_bitcoin_wallet(ctx).await
    }
}

pub struct GetConfigArgs;

impl Request for GetConfigArgs {
    type Response = serde_json::Value;

    async fn request(self, ctx: Arc<Context>) -> Result<Self::Response> {
        get_config(ctx).await
    }
}

#[derive(Debug, Default, Clone)]
pub struct GetSwapInfosAllArgs;

impl Request for GetSwapInfosAllArgs {
    type Response = Vec<GetSwapInfoResponse>;

    async fn request(self, ctx: Arc<Context>) -> Result<Self::Response> {
        get_swap_infos_all(ctx).await
    }
}

#[typeshare]
#[derive(Serialize, Deserialize, Debug)]
pub struct GetLogsArgs {
    pub swap_id: Option<Uuid>,
    pub redact: bool,
    #[typeshare(serialized_as = "string")]
    pub logs_dir: Option<PathBuf>,
}

#[typeshare]
#[derive(Serialize, Debug)]
pub struct GetLogsResponse {
    logs: Vec<String>,
}

impl Request for GetLogsArgs {
    type Response = GetLogsResponse;

    async fn request(self, ctx: Arc<Context>) -> Result<Self::Response> {
        let dir = self.logs_dir.unwrap_or(ctx.config.data_dir.join("logs"));
        let logs = get_logs(dir, self.swap_id, self.redact).await?;

        for msg in &logs {
            println!("{msg}");
        }

        Ok(GetLogsResponse { logs })
    }
}

#[tracing::instrument(fields(method = "suspend_current_swap"), skip(context))]
pub async fn suspend_current_swap(context: Arc<Context>) -> Result<SuspendCurrentSwapResponse> {
    let swap_id = context.swap_lock.get_current_swap_id().await;

    if let Some(id_value) = swap_id {
        context.swap_lock.send_suspend_signal().await?;

        Ok(SuspendCurrentSwapResponse { swap_id: id_value })
    } else {
        bail!("No swap is currently running")
    }
}

#[tracing::instrument(fields(method = "get_swap_infos_all"), skip(context))]
pub async fn get_swap_infos_all(context: Arc<Context>) -> Result<Vec<GetSwapInfoResponse>> {
    let swap_ids = context.db.all().await?;
    let mut swap_infos = Vec::new();

    for (swap_id, _) in swap_ids {
        match get_swap_info(GetSwapInfoArgs { swap_id }, context.clone()).await {
            Ok(swap_info) => swap_infos.push(swap_info),
            Err(error) => {
                tracing::warn!(%swap_id, "Failed to get swap info: {:#}", error);
            }
        }
    }

    Ok(swap_infos)
}

#[tracing::instrument(fields(method = "get_swap_info"), skip(context))]
pub async fn get_swap_info(
    args: GetSwapInfoArgs,
    context: Arc<Context>,
) -> Result<GetSwapInfoResponse> {
    let bitcoin_wallet = context
        .bitcoin_wallet
        .as_ref()
        .context("Could not get Bitcoin wallet")?;

    let state = context.db.get_state(args.swap_id).await?;
    let is_completed = state.swap_finished();

    let peer_id = context
        .db
        .get_peer_id(args.swap_id)
        .await
        .with_context(|| "Could not get PeerID")?;

    let addresses = context
        .db
        .get_addresses(peer_id)
        .await
        .with_context(|| "Could not get addressess")?;

    let start_date = context.db.get_swap_start_date(args.swap_id).await?;

    let swap_state: BobState = state.try_into()?;

    let (
        bdx_amount,
        btc_amount,
        tx_lock_id,
        tx_cancel_fee,
        tx_refund_fee,
        tx_lock_fee,
        btc_refund_address,
        cancel_timelock,
        punish_timelock,
    ) = context
        .db
        .get_states(args.swap_id)
        .await?
        .iter()
        .find_map(|state| {
            if let State::Bob(BobState::SwapSetupCompleted(state2)) = state {
                let bdx_amount = state2.bdx;
                let btc_amount = state2.tx_lock.lock_amount();
                let tx_cancel_fee = state2.tx_cancel_fee;
                let tx_refund_fee = state2.tx_refund_fee;
                let tx_lock_id = state2.tx_lock.txid();
                let btc_refund_address = state2.refund_address.to_string();

                if let Ok(tx_lock_fee) = state2.tx_lock.fee() {
                    Some((
                        bdx_amount,
                        btc_amount,
                        tx_lock_id,
                        tx_cancel_fee,
                        tx_refund_fee,
                        tx_lock_fee,
                        btc_refund_address,
                        state2.cancel_timelock,
                        state2.punish_timelock,
                    ))
                } else {
                    None
                }
            } else {
                None
            }
        })
        .with_context(|| "Did not find SwapSetupCompleted state for swap")?;

    let timelock = match swap_state.clone() {
         BobState::Started { .. } | BobState::SafelyAborted | BobState::SwapSetupCompleted(_) => {
            None
        }
        BobState::BtcLocked { state3: state, .. }
        | BobState::BeldexLockProofReceived { state, .. } => {
            Some(state.expired_timelock(bitcoin_wallet).await?)
        }
        BobState::BeldexLocked(state) | BobState::EncSigSent(state) => {
            Some(state.expired_timelock(bitcoin_wallet).await?)
        }
        BobState::CancelTimelockExpired(state) | BobState::BtcCancelled(state) => {
            Some(state.expired_timelock(bitcoin_wallet).await?)
        }
        BobState::BtcPunished { .. } => Some(ExpiredTimelocks::Punish),
        BobState::BtcRefunded(_) | BobState::BtcRedeemed(_) | BobState::BeldexRedeemed { .. } => None,
    };

    Ok(GetSwapInfoResponse {
        swap_id: args.swap_id,
        seller: Seller {
            peer_id,
            addresses: addresses.iter().map(|a| a.to_string()).collect(),
        },
        completed: is_completed,
        start_date,
        state_name: format!("{}", swap_state),
        bdx_amount,
        btc_amount,
        tx_lock_id,
        tx_cancel_fee,
        tx_refund_fee,
        tx_lock_fee,
        btc_refund_address: btc_refund_address.to_string(),
        cancel_timelock,
        punish_timelock,
        timelock,
    })
}

#[tracing::instrument(fields(method = "buy_beldex"), skip(context))]
pub async fn buy_beldex(
    buy_beldex: BuyBeldexArgs,
    context: Arc<Context>,
) -> Result<BuyBeldexResponse, anyhow::Error> {
    let BuyBeldexArgs {
        seller,
        bitcoin_change_address,
        beldex_receive_address,
        amount: preferred_amount,
    } = buy_beldex;

    let swap_id = Uuid::new_v4();

    let bitcoin_wallet = Arc::clone(
        context
            .bitcoin_wallet
            .as_ref()
            .expect("Could not find Bitcoin wallet"),
    );
    let beldex_wallet = Arc::clone(
        context
            .beldex_wallet
            .as_ref()
            .context("Could not get Beldex wallet")?,
    );
    let env_config = context.config.env_config;
    let seed = context.config.seed.clone().context("Could not get seed")?;

    let seller_peer_id = seller
        .extract_peer_id()
        .context("Seller address must contain peer ID")?;
    context
        .db
        .insert_address(seller_peer_id, seller.clone())
        .await?;

    let behaviour = cli::Behaviour::new(
        seller_peer_id,
        env_config,
        bitcoin_wallet.clone(),
        (seed.derive_libp2p_identity(), context.config.namespace),
    );
    let mut swarm = swarm::cli(
        seed.derive_libp2p_identity(),
        context.config.tor_socks5_port,
        behaviour,
    )
    .await?;

    swarm.behaviour_mut().add_address(seller_peer_id, seller);

    context
        .db
        .insert_beldex_address(swap_id, beldex_receive_address)
        .await?;

    tracing::debug!(peer_id = %swarm.local_peer_id(), "Network layer initialized");

    context.swap_lock.acquire_swap_lock(swap_id).await?;

    let initialize_swap = tokio::select! {
        biased;
        _ = context.swap_lock.listen_for_swap_force_suspension() => {
            tracing::debug!("Shutdown signal received, exiting");
            context.swap_lock.release_swap_lock().await.expect("Shutdown signal received but failed to release swap lock. The swap process has been terminated but the swap lock is still active.");

            context.tauri_handle.emit_swap_progress_event(swap_id, TauriSwapProgressEvent::Released);

            bail!("Shutdown signal received");
        },
        result = async {
            let (event_loop, mut event_loop_handle) =
                EventLoop::new(swap_id, swarm, seller_peer_id, context.db.clone())?;
            let event_loop = tokio::spawn(event_loop.run().in_current_span());

            let bid_quote = event_loop_handle.request_quote().await?;

            Ok::<_, anyhow::Error>((event_loop, event_loop_handle, bid_quote))
        } => {
            result
        },
    };

    let (event_loop, event_loop_handle, bid_quote) = match initialize_swap {
        Ok(result) => result,
        Err(error) => {
            tracing::error!(%swap_id, "Swap initialization failed: {:#}", error);
            context
                .swap_lock
                .release_swap_lock()
                .await
                .expect("Could not release swap lock");

            context
                .tauri_handle
                .emit_swap_progress_event(swap_id, TauriSwapProgressEvent::Released);

            bail!(error);
        }
    };

    context
        .tauri_handle
        .emit_swap_progress_event(swap_id, TauriSwapProgressEvent::ReceivedQuote(bid_quote));

    context.tasks.clone().spawn(async move {
        tokio::select! {
            biased;
            _ = context.swap_lock.listen_for_swap_force_suspension() => {
                tracing::debug!("Shutdown signal received, exiting");
                context.swap_lock.release_swap_lock().await.expect("Shutdown signal received but failed to release swap lock. The swap process has been terminated but the swap lock is still active.");

                context.tauri_handle.emit_swap_progress_event(swap_id, TauriSwapProgressEvent::Released);

                bail!("Shutdown signal received");
            },
            event_loop_result = event_loop => {
                match event_loop_result {
                    Ok(_) => {
                        tracing::debug!(%swap_id, "EventLoop completed")
                    }
                    Err(error) => {
                        tracing::error!(%swap_id, "EventLoop failed: {:#}", error)
                    }
                }
            },
            swap_result = async {
                let max_givable = || bitcoin_wallet.max_giveable(TxLock::script_size());
                let estimate_fee = |amount| bitcoin_wallet.estimate_fee(TxLock::weight(), amount);

                let determine_amount = determine_btc_to_swap(
                    context.config.json,
                    bid_quote,
                    bitcoin_wallet.new_address(),
                    || bitcoin_wallet.balance(),
                    max_givable,
                    || bitcoin_wallet.sync(),
                    estimate_fee,
                    context.tauri_handle.clone(),
                    Some(swap_id),
                    preferred_amount,
                );

                let (amount, fees) = match determine_amount.await {
                    Ok(val) => val,
                    Err(error) => match error.downcast::<ZeroQuoteReceived>() {
                        Ok(_) => {
                            bail!("Seller's Beldex balance is currently too low to initiate a swap, please try again later")
                        }
                        Err(other) => bail!(other),
                    },
                };

                tracing::info!(%amount, %fees,  "Determined swap amount");

                context.db.insert_peer_id(swap_id, seller_peer_id).await?;

                let swap = Swap::new(
                    Arc::clone(&context.db),
                    swap_id,
                    Arc::clone(&bitcoin_wallet),
                    beldex_wallet,
                    env_config,
                    event_loop_handle,
                    beldex_receive_address,
                    bitcoin_change_address,
                    amount,
                ).with_event_emitter(context.tauri_handle.clone());

                bob::run(swap).await
            } => {
                match swap_result {
                    Ok(state) => {
                        tracing::debug!(%swap_id, state=%state, "Swap completed")
                    }
                    Err(error) => {
                        tracing::error!(%swap_id, "Failed to complete swap: {:#}", error)
                    }
                }
            },
        };
        tracing::debug!(%swap_id, "Swap completed");

        context
            .swap_lock
            .release_swap_lock()
            .await
            .expect("Could not release swap lock");

        context.tauri_handle.emit_swap_progress_event(swap_id, TauriSwapProgressEvent::Released);

        Ok::<_, anyhow::Error>(())
    }.in_current_span()).await;

    Ok(BuyBeldexResponse {
        swap_id,
        quote: bid_quote,
    })
}

#[tracing::instrument(fields(method = "resume_swap"), skip(context))]
pub async fn resume_swap(
    resume: ResumeSwapArgs,
    context: Arc<Context>,
) -> Result<ResumeSwapResponse> {
    let ResumeSwapArgs { swap_id } = resume;
    context.swap_lock.acquire_swap_lock(swap_id).await?;

    let seller_peer_id = context.db.get_peer_id(swap_id).await?;
    let seller_addresses = context.db.get_addresses(seller_peer_id).await?;

    let seed = context
        .config
        .seed
        .as_ref()
        .context("Could not get seed")?
        .derive_libp2p_identity();

    let behaviour = cli::Behaviour::new(
        seller_peer_id,
        context.config.env_config,
        Arc::clone(
            context
                .bitcoin_wallet
                .as_ref()
                .context("Could not get Bitcoin wallet")?,
        ),
        (seed.clone(), context.config.namespace),
    );
    let mut swarm = swarm::cli(seed.clone(), context.config.tor_socks5_port, behaviour).await?;
    let our_peer_id = swarm.local_peer_id();

    tracing::debug!(peer_id = %our_peer_id, "Network layer initialized");

    for seller_address in seller_addresses {
        swarm
            .behaviour_mut()
            .add_address(seller_peer_id, seller_address);
    }

    let (event_loop, event_loop_handle) =
        EventLoop::new(swap_id, swarm, seller_peer_id, context.db.clone())?;
    let beldex_receive_address = context.db.get_beldex_address(swap_id).await?;
    let swap = Swap::from_db(
        Arc::clone(&context.db),
        swap_id,
        Arc::clone(
            context
                .bitcoin_wallet
                .as_ref()
                .context("Could not get Bitcoin wallet")?,
        ),
        Arc::clone(
            context
                .beldex_wallet
                .as_ref()
                .context("Could not get Beldex wallet")?,
        ),
        context.config.env_config,
        event_loop_handle,
        beldex_receive_address,
    )
    .await?
    .with_event_emitter(context.tauri_handle.clone());

    context.tasks.clone().spawn(
        async move {
            let handle = tokio::spawn(event_loop.run().in_current_span());
            tokio::select! {
                biased;
                _ = context.swap_lock.listen_for_swap_force_suspension() => {
                     tracing::debug!("Shutdown signal received, exiting");
                    context.swap_lock.release_swap_lock().await.expect("Shutdown signal received but failed to release swap lock. The swap process has been terminated but the swap lock is still active.");

                    context.tauri_handle.emit_swap_progress_event(swap_id, TauriSwapProgressEvent::Released);

                    bail!("Shutdown signal received");
                },

                event_loop_result = handle => {
                    match event_loop_result {
                        Ok(_) => {
                            tracing::debug!(%swap_id, "EventLoop completed during swap resume")
                        }
                        Err(error) => {
                            tracing::error!(%swap_id, "EventLoop failed during swap resume: {:#}", error)
                        }
                    }
                },
                swap_result = bob::run(swap) => {
                    match swap_result {
                        Ok(state) => {
                            tracing::debug!(%swap_id, state=%state, "Swap completed after resuming")
                        }
                        Err(error) => {
                            tracing::error!(%swap_id, "Failed to resume swap: {:#}", error)
                        }
                    }

                }
            }
            context
                .swap_lock
                .release_swap_lock()
                .await
                .expect("Could not release swap lock");

            context.tauri_handle.emit_swap_progress_event(swap_id, TauriSwapProgressEvent::Released);

            Ok::<(), anyhow::Error>(())
        }
        .in_current_span(),
    ).await;

    Ok(ResumeSwapResponse {
        result: "OK".to_string(),
    })
}

#[tracing::instrument(fields(method = "cancel_and_refund"), skip(context))]
pub async fn cancel_and_refund(
    cancel_and_refund: CancelAndRefundArgs,
    context: Arc<Context>,
) -> Result<serde_json::Value> {
    let CancelAndRefundArgs { swap_id } = cancel_and_refund;
    let bitcoin_wallet = context
        .bitcoin_wallet
        .as_ref()
        .context("Could not get Bitcoin wallet")?;

    context.swap_lock.acquire_swap_lock(swap_id).await?;

    let state =
        cli::cancel_and_refund(swap_id, Arc::clone(bitcoin_wallet), Arc::clone(&context.db)).await;

    context
        .swap_lock
        .release_swap_lock()
        .await
        .expect("Could not release swap lock");

    context
        .tauri_handle
        .emit_swap_progress_event(swap_id, TauriSwapProgressEvent::Released);

    state.map(|state| {
        json!({
            "result": state,
        })
    })
}

#[tracing::instrument(fields(method = "get_history"), skip(context))]
pub async fn get_history(context: Arc<Context>) -> Result<GetHistoryResponse> {
    let swaps = context.db.all().await?;
    let mut vec: Vec<GetHistoryEntry> = Vec::new();
    for (swap_id, state) in swaps {
        let state: BobState = state.try_into()?;
        vec.push(GetHistoryEntry {
            swap_id,
            state: state.to_string(),
        })
    }

    Ok(GetHistoryResponse { swaps: vec })
}

#[tracing::instrument(fields(method = "get_config"), skip(context))]
pub async fn get_config(context: Arc<Context>) -> Result<serde_json::Value> {
    let data_dir_display = context.config.data_dir.display();
    tracing::info!(path=%data_dir_display, "Data directory");
    tracing::info!(path=%format!("{}/logs", data_dir_display), "Log files directory");
    tracing::info!(path=%format!("{}/sqlite", data_dir_display), "Sqlite file location");
    tracing::info!(path=%format!("{}/seed.pem", data_dir_display), "Seed file location");
    tracing::info!(path=%format!("{}/beldex", data_dir_display), "Beldex-wallet-rpc directory");
    tracing::info!(path=%format!("{}/wallet", data_dir_display), "Internal bitcoin wallet directory");

    Ok(json!({
        "log_files": format!("{}/logs", data_dir_display),
        "sqlite": format!("{}/sqlite", data_dir_display),
        "seed": format!("{}/seed.pem", data_dir_display),
        "beldex-wallet-rpc": format!("{}/beldex", data_dir_display),
        "bitcoin_wallet": format!("{}/wallet", data_dir_display),
    }))
}

#[tracing::instrument(fields(method = "withdraw_btc"), skip(context))]
pub async fn withdraw_btc(
    withdraw_btc: WithdrawBtcArgs,
    context: Arc<Context>,
) -> Result<WithdrawBtcResponse> {
    let WithdrawBtcArgs { address, amount } = withdraw_btc;
    let bitcoin_wallet = context
        .bitcoin_wallet
        .as_ref()
        .context("Could not get Bitcoin wallet")?;

    let amount = match amount {
        Some(amount) => amount,
        None => {
            bitcoin_wallet
                .max_giveable(address.script_pubkey().len())
                .await?
        }
    };
    let psbt = bitcoin_wallet
        .send_to_address(address, amount, None)
        .await?;
    let signed_tx = bitcoin_wallet.sign_and_finalize(psbt).await?;

    bitcoin_wallet
        .broadcast(signed_tx.clone(), "withdraw")
        .await?;

    Ok(WithdrawBtcResponse {
        txid: signed_tx.txid().to_string(),
        amount,
    })
}

#[tracing::instrument(fields(method = "start_daemon"), skip(context))]
pub async fn start_daemon(
    start_daemon: StartDaemonArgs,
    context: Context,
) -> Result<serde_json::Value> {
    let StartDaemonArgs { server_address } = start_daemon;
    tracing::info!("Attempting to start daemon...");
    // Default to 127.0.0.1:1234
    let server_address = server_address.unwrap_or("127.0.0.1:1234".parse()?);

    tracing::info!(%server_address, "Running RPC server...");
    let (addr, server_handle) = rpc::run_server(server_address, context.clone()).await?;

    tracing::info!(%addr, "Successfully started RPC server");

    // Store the handle in context for graceful shutdown
    *context.rpc_server_handle.lock().await = Some(server_handle.clone());

    // We spawn a task to wait for the server to stop, but we return the address immediately
    tokio::spawn(async move {
        server_handle.stopped().await;
        tracing::info!("Stopped RPC server");
        let _ = context.cleanup();
    });

    Ok(json!({
        "server_address": addr.to_string(),
    }))
}

#[tracing::instrument(fields(method = "stop_daemon"), skip(context))]
pub async fn stop_daemon(context: Arc<Context>) -> Result<serde_json::Value> {
    // 1. Try to stop gracefully via the server handle
    let mut handle_lock = context.rpc_server_handle.lock().await;
    if let Some(handle) = handle_lock.take() {
        tracing::info!("Stopping RPC server via handle...");
        let _ = handle.stop();
        // The spawned task in start_daemon will handle context.cleanup()
    } else {
        // 2. Fallback to pkill if handle is missing
        tracing::warn!("Server handle not found, falling back to pkill");
        use std::process::Command;
        let _ = Command::new("pkill")
            .arg("-f")
            .arg("beldex-wallet-rpc")
            .output();
        let _ = context.cleanup();
    }

    Ok(json!({ "result": "OK" }))
}

#[tracing::instrument(fields(method = "open_data_dir"), skip(context))]
pub async fn open_data_dir(context: Arc<Context>) -> Result<serde_json::Value> {
    use std::process::Command;
    let path = context.config.data_dir.clone();

    tracing::info!(path = %path.display(), "Opening data directory");

    // On Linux, use xdg-open
    Command::new("xdg-open")
        .arg(path)
        .spawn()
        .context("Failed to spawn xdg-open")?;

    Ok(json!({ "result": "OK" }))
}

#[tracing::instrument(fields(method = "get_balance"), skip(context))]
pub async fn get_balance(balance: BalanceArgs, context: Arc<Context>) -> Result<BalanceResponse> {
    let BalanceArgs { force_refresh } = balance;
    let bitcoin_wallet = context
        .bitcoin_wallet
        .as_ref()
        .context("Could not get Bitcoin wallet")?;

    if force_refresh {
        bitcoin_wallet.sync().await?;
    }

    let bitcoin_balance = bitcoin_wallet.balance().await?;

    if force_refresh {
        tracing::info!(
            balance = %bitcoin_balance,
            "Checked Bitcoin balance",
        );
    } else {
        tracing::debug!(
            balance = %bitcoin_balance,
            "Current Bitcoin balance as of last sync",
        );
    }

    Ok(BalanceResponse {
        balance: bitcoin_balance,
    })
}

#[tracing::instrument(fields(method = "list_sellers"), skip(context))]
pub async fn list_sellers(
    list_sellers: ListSellersArgs,
    context: Arc<Context>,
) -> Result<serde_json::Value> {
    let ListSellersArgs { rendezvous_point } = list_sellers;
    let rendezvous_node_peer_id = rendezvous_point
        .extract_peer_id()
        .context("Rendezvous node address must contain peer ID")?;

    let identity = context
        .config
        .seed
        .as_ref()
        .context("Cannot extract seed")?
        .derive_libp2p_identity();

    let sellers = list_sellers_impl(
        rendezvous_node_peer_id,
        rendezvous_point,
        context.config.namespace,
        context.config.tor_socks5_port,
        identity,
    )
    .await?;

    for seller in &sellers {
        match seller.status {
            SellerStatus::Online(quote) => {
                tracing::info!(
                    price = %quote.price.to_string(),
                    min_quantity = %quote.min_quantity.to_string(),
                    max_quantity = %quote.max_quantity.to_string(),
                    status = "Online",
                    address = %seller.multiaddr.to_string(),
                    "Fetched peer status"
                );
            }
            SellerStatus::Unreachable => {
                tracing::info!(
                    status = "Unreachable",
                    address = %seller.multiaddr.to_string(),
                    "Fetched peer status"
                );
            }
        }
    }

    Ok(json!({ "sellers": sellers }))
}

#[tracing::instrument(fields(method = "export_bitcoin_wallet"), skip(context))]
pub async fn export_bitcoin_wallet(context: Arc<Context>) -> Result<serde_json::Value> {
    let bitcoin_wallet = context
        .bitcoin_wallet
        .as_ref()
        .context("Could not get Bitcoin wallet")?;

    let wallet_export = bitcoin_wallet.wallet_export("cli").await?;
    tracing::info!(descriptor=%wallet_export.to_string(), "Exported bitcoin wallet");
    Ok(json!({
        "descriptor": wallet_export.to_string(),
    }))
}

#[tracing::instrument(fields(method = "beldex_recovery"), skip(context))]
pub async fn beldex_recovery(
    beldex_recovery: BeldexRecoveryArgs,
    context: Arc<Context>,
) -> Result<serde_json::Value> {
    let BeldexRecoveryArgs { swap_id } = beldex_recovery;
    let swap_state: BobState = context.db.get_state(swap_id).await?.try_into()?;

    if let BobState::BtcRedeemed(state5) = swap_state {
        let (spend_key, view_key) = state5.bdx_keys();
        let restore_height = state5.beldex_wallet_restore_blockheight.height;

        let address = beldex::Address::standard(
            context.config.env_config.beldex_network,
            beldex::PublicKey::from_private_key(&spend_key),
            beldex::PublicKey::from(view_key.public()),
        );

        tracing::info!(restore_height=%restore_height, address=%address, spend_key=%spend_key, view_key=%view_key, "Beldex recovery information");

        Ok(json!({
            "address": address,
            "spend_key": spend_key.to_string(),
            "view_key": view_key.to_string(),
            "restore_height": state5.beldex_wallet_restore_blockheight.height,
        }))
    } else {
        bail!(
            "Cannot print beldex recovery information in state {}, only possible for BtcRedeemed",
            swap_state
        )
    }
}

#[tracing::instrument(fields(method = "get_current_swap"), skip(context))]
pub async fn get_current_swap(context: Arc<Context>) -> Result<serde_json::Value> {
    Ok(json!({
        "swap_id": context.swap_lock.get_current_swap_id().await
    }))
}

fn qr_code(value: &impl ToString) -> Result<String> {
    let code = QrCode::new(value.to_string())?;
    let qr_code = code
        .render::<unicode::Dense1x2>()
        .dark_color(unicode::Dense1x2::Light)
        .light_color(unicode::Dense1x2::Dark)
        .build();
    Ok(qr_code)
}

#[allow(clippy::too_many_arguments)]
pub async fn determine_btc_to_swap<FB, TB, FMG, TMG, FS, TS, FFE, TFE>(
    json: bool,
    bid_quote: BidQuote,
    get_new_address: impl Future<Output = Result<bitcoin::Address>>,
    balance: FB,
    max_giveable_fn: FMG,
    sync: FS,
    estimate_fee: FFE,
    event_emitter: Option<TauriHandle>,
    swap_id: Option<Uuid>,
    preferred_amount: Option<bitcoin::Amount>,
) -> Result<(bitcoin::Amount, bitcoin::Amount)>
where
    TB: Future<Output = Result<bitcoin::Amount>>,
    FB: Fn() -> TB,
    TMG: Future<Output = Result<bitcoin::Amount>>,
    FMG: Fn() -> TMG,
    TS: Future<Output = Result<()>>,
    FS: Fn() -> TS,
    FFE: Fn(bitcoin::Amount) -> TFE,
    TFE: Future<Output = Result<bitcoin::Amount>>,
{
    if bid_quote.max_quantity == bitcoin::Amount::ZERO {
        bail!(ZeroQuoteReceived)
    }

    tracing::info!(
        price = %bid_quote.price,
        minimum_amount = %bid_quote.min_quantity,
        maximum_amount = %bid_quote.max_quantity,
        "Received quote",
    );

    sync().await?;
    let mut max_giveable = max_giveable_fn().await?;

    if max_giveable == bitcoin::Amount::ZERO || max_giveable < bid_quote.min_quantity {
        let deposit_address = get_new_address.await?;
        let minimum_amount = bid_quote.min_quantity;
        let maximum_amount = bid_quote.max_quantity;

        if !json {
            eprintln!("{}", qr_code(&deposit_address)?);
        }

        loop {
            let min_outstanding = bid_quote.min_quantity - max_giveable;
            let min_bitcoin_lock_tx_fee = estimate_fee(min_outstanding).await?;
            let min_deposit_until_swap_will_start = min_outstanding + min_bitcoin_lock_tx_fee;
            let max_deposit_until_maximum_amount_is_reached =
                maximum_amount - max_giveable + min_bitcoin_lock_tx_fee;

            tracing::info!(
                "Deposit at least {} to cover the min quantity with fee!",
                min_deposit_until_swap_will_start
            );
            tracing::info!(
                %deposit_address,
                %min_deposit_until_swap_will_start,
                %max_deposit_until_maximum_amount_is_reached,
                %max_giveable,
                %minimum_amount,
                %maximum_amount,
                %min_bitcoin_lock_tx_fee,
                price = %bid_quote.price,
                "Waiting for Bitcoin deposit",
            );

            if let Some(swap_id) = swap_id {
                event_emitter.emit_swap_progress_event(
                    swap_id,
                    TauriSwapProgressEvent::WaitingForBtcDeposit {
                        deposit_address: deposit_address.clone(),
                        max_giveable,
                        min_deposit_until_swap_will_start,
                        max_deposit_until_maximum_amount_is_reached,
                        min_bitcoin_lock_tx_fee,
                        quote: bid_quote,
                    },
                );
            }

            max_giveable = loop {
                sync().await?;
                let new_max_givable = max_giveable_fn().await?;

                if new_max_givable > max_giveable {
                    break new_max_givable;
                }

                tokio::time::sleep(Duration::from_secs(1)).await;
            };

            let new_balance = balance().await?;
            tracing::info!(%new_balance, %max_giveable, "Received Bitcoin");

            if max_giveable < bid_quote.min_quantity {
                tracing::info!("Deposited amount is not enough to cover `min_quantity` when accounting for network fees");
                continue;
            }

            break;
        }
    };

    let balance = balance().await?;
    let fees = balance - max_giveable;
    let max_accepted = bid_quote.max_quantity;

    let btc_swap_amount = match preferred_amount {
        Some(preferred) => {
            let limit = min(preferred, max_accepted);
            min(max_giveable, limit)
        }
        None => min(max_giveable, max_accepted),
    };

    Ok((btc_swap_amount, fees))
}
