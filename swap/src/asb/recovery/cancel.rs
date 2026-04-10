use crate::bitcoin::{parse_rpc_error_code, RpcErrorCode, Txid, Wallet};
use crate::protocol::alice::AliceState;
use crate::protocol::Database;
use anyhow::{bail, Result};
use std::convert::TryInto;
use std::sync::Arc;
use uuid::Uuid;

pub async fn cancel(
    swap_id: Uuid,
    bitcoin_wallet: Arc<Wallet>,
    db: Arc<dyn Database>,
) -> Result<(Txid, AliceState)> {
    let state = db.get_state(swap_id).await?.try_into()?;

    let (beldex_wallet_restore_blockheight, transfer_proof, state3) = match state {

        // In case no BDX has been locked, move to Safely Aborted
        AliceState::Started { .. }
        | AliceState::BtcLockTransactionSeen { .. }
        | AliceState::BtcLocked { .. } => bail!("Cannot cancel swap {} because it is in state {} where no BDX was locked.", swap_id, state),

        AliceState::BeldexLockTransactionSent { beldex_wallet_restore_blockheight, transfer_proof, state3,  }
        | AliceState::BeldexLocked { beldex_wallet_restore_blockheight, transfer_proof, state3 }
        | AliceState::BeldexLockTransferProofSent { beldex_wallet_restore_blockheight, transfer_proof, state3 }
        // in cancel mode we do not care about the fact that we could redeem, but always wait for cancellation (leading either refund or punish)
        | AliceState::EncSigLearned { beldex_wallet_restore_blockheight, transfer_proof, state3, .. }
        | AliceState::CancelTimelockExpired { beldex_wallet_restore_blockheight, transfer_proof, state3}
        | AliceState::BtcCancelled { beldex_wallet_restore_blockheight, transfer_proof, state3 }
        | AliceState::BtcRefunded { beldex_wallet_restore_blockheight, transfer_proof,  state3 ,.. }
        | AliceState::BtcPunishable { beldex_wallet_restore_blockheight, transfer_proof, state3 }  => {
            (beldex_wallet_restore_blockheight, transfer_proof, state3)
        }

        // The redeem transaction was already published, it is not safe to cancel anymore
        AliceState::BtcRedeemTransactionPublished { .. } => bail!(" The redeem transaction was already published, it is not safe to cancel anymore"),

        // Alice already in final state
        | AliceState::BtcRedeemed
        | AliceState::BeldexRefunded
        | AliceState::BtcPunished { .. }
        | AliceState::SafelyAborted => bail!("Swap is in state {} which is not cancelable", state),
    };

    let txid = match state3.submit_tx_cancel(bitcoin_wallet.as_ref()).await {
        Ok(txid) => txid,
        Err(err) => {
            if let Ok(code) = parse_rpc_error_code(&err) {
                if code == i64::from(RpcErrorCode::RpcVerifyAlreadyInChain) {
                    tracing::info!("Cancel transaction has already been confirmed on chain")
                }
            }
            bail!(err);
        }
    };

    let state = AliceState::BtcCancelled {
        beldex_wallet_restore_blockheight,
        transfer_proof,
        state3,
    };
    db.insert_latest_state(swap_id, state.clone().into())
        .await?;

    Ok((txid, state))
}
