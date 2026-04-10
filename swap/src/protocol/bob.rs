use std::sync::Arc;

use anyhow::Result;
use uuid::Uuid;

use crate::cli::api::tauri_bindings::TauriHandle;
use crate::protocol::Database;
use crate::{bitcoin, cli, env, beldex};

pub use self::state::*;
pub use self::swap::{run, run_until};
use std::convert::TryInto;

pub mod state;
pub mod swap;

pub struct Swap {
    pub state: BobState,
    pub event_loop_handle: cli::EventLoopHandle,
    pub db: Arc<dyn Database + Send + Sync>,
    pub bitcoin_wallet: Arc<bitcoin::Wallet>,
    pub beldex_wallet: Arc<beldex::Wallet>,
    pub env_config: env::Config,
    pub id: Uuid,
    pub beldex_receive_address: beldex::Address,
    pub event_emitter: Option<TauriHandle>,
}

impl Swap {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        db: Arc<dyn Database + Send + Sync>,
        id: Uuid,
        bitcoin_wallet: Arc<bitcoin::Wallet>,
        beldex_wallet: Arc<beldex::Wallet>,
        env_config: env::Config,
        event_loop_handle: cli::EventLoopHandle,
        beldex_receive_address: beldex::Address,
        bitcoin_change_address: bitcoin::Address,
        btc_amount: bitcoin::Amount,
    ) -> Self {
        Self {
            state: BobState::Started {
                btc_amount,
                change_address: bitcoin_change_address,
            },
            event_loop_handle,
            db,
            bitcoin_wallet,
            beldex_wallet,
            env_config,
            id,
            beldex_receive_address,
            event_emitter: None,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn from_db(
        db: Arc<dyn Database + Send + Sync>,
        id: Uuid,
        bitcoin_wallet: Arc<bitcoin::Wallet>,
        beldex_wallet: Arc<beldex::Wallet>,
        env_config: env::Config,
        event_loop_handle: cli::EventLoopHandle,
        beldex_receive_address: beldex::Address,
    ) -> Result<Self> {
        let state = db.get_state(id).await?.try_into()?;

        Ok(Self {
            state,
            event_loop_handle,
            db,
            bitcoin_wallet,
            beldex_wallet,
            env_config,
            id,
            beldex_receive_address,
            event_emitter: None,
        })
    }

    pub fn with_event_emitter(mut self, event_emitter: Option<TauriHandle>) -> Self {
        self.event_emitter = event_emitter;
        self
    }
}
