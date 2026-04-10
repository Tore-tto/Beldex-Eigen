use crate::bitcoin::EncryptedSignature;
use crate::beldex;
use crate::beldex::{beldex_private_key, TransferProof};
use crate::protocol::alice;
use crate::protocol::alice::AliceState;
use beldex_rpc::wallet::BlockHeight;
use serde::{Deserialize, Serialize};
use std::fmt;

// Large enum variant is fine because this is only used for database
// and is dropped once written in DB.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub enum Alice {
    Started {
        state3: alice::State3,
    },
    BtcLockTransactionSeen {
        state3: alice::State3,
    },
    BtcLocked {
        state3: alice::State3,
    },
    BeldexLockTransactionSent {
        beldex_wallet_restore_blockheight: BlockHeight,
        transfer_proof: TransferProof,
        state3: alice::State3,
    },
    BeldexLocked {
        beldex_wallet_restore_blockheight: BlockHeight,
        transfer_proof: TransferProof,
        state3: alice::State3,
    },
    BeldexLockTransferProofSent {
        beldex_wallet_restore_blockheight: BlockHeight,
        transfer_proof: TransferProof,
        state3: alice::State3,
    },
    EncSigLearned {
        beldex_wallet_restore_blockheight: BlockHeight,
        transfer_proof: TransferProof,
        encrypted_signature: EncryptedSignature,
        state3: alice::State3,
    },
    BtcRedeemTransactionPublished {
        state3: alice::State3,
    },
    CancelTimelockExpired {
        beldex_wallet_restore_blockheight: BlockHeight,
        transfer_proof: TransferProof,
        state3: alice::State3,
    },
    BtcCancelled {
        beldex_wallet_restore_blockheight: BlockHeight,
        transfer_proof: TransferProof,
        state3: alice::State3,
    },
    BtcPunishable {
        beldex_wallet_restore_blockheight: BlockHeight,
        transfer_proof: TransferProof,
        state3: alice::State3,
    },
    BtcRefunded {
        beldex_wallet_restore_blockheight: BlockHeight,
        transfer_proof: TransferProof,
        state3: alice::State3,
        #[serde(with = "beldex_private_key")]
        spend_key: beldex::PrivateKey,
    },
    Done(AliceEndState),
}

#[derive(Clone, strum::Display, Debug, Deserialize, Serialize, PartialEq)]
pub enum AliceEndState {
    SafelyAborted,
    BtcRedeemed,
    BeldexRefunded,
    BtcPunished { state3: alice::State3 },
}

impl From<AliceState> for Alice {
    fn from(alice_state: AliceState) -> Self {
        match alice_state {
            AliceState::Started { state3 } => Alice::Started {
                state3: state3.as_ref().clone(),
            },
            AliceState::BtcLockTransactionSeen { state3 } => Alice::BtcLockTransactionSeen {
                state3: state3.as_ref().clone(),
            },
            AliceState::BtcLocked { state3 } => Alice::BtcLocked {
                state3: state3.as_ref().clone(),
            },
            AliceState::BeldexLockTransactionSent {
                beldex_wallet_restore_blockheight,
                transfer_proof,
                state3,
            } => Alice::BeldexLockTransactionSent {
                beldex_wallet_restore_blockheight,
                transfer_proof,
                state3: state3.as_ref().clone(),
            },
            AliceState::BeldexLocked {
                beldex_wallet_restore_blockheight,
                transfer_proof,
                state3,
            } => Alice::BeldexLocked {
                beldex_wallet_restore_blockheight,
                transfer_proof,
                state3: state3.as_ref().clone(),
            },
            AliceState::BeldexLockTransferProofSent {
                beldex_wallet_restore_blockheight,
                transfer_proof,
                state3,
            } => Alice::BeldexLockTransferProofSent {
                beldex_wallet_restore_blockheight,
                transfer_proof,
                state3: state3.as_ref().clone(),
            },
            AliceState::EncSigLearned {
                beldex_wallet_restore_blockheight,
                transfer_proof,
                state3,
                encrypted_signature,
            } => Alice::EncSigLearned {
                beldex_wallet_restore_blockheight,
                transfer_proof,
                state3: state3.as_ref().clone(),
                encrypted_signature: encrypted_signature.as_ref().clone(),
            },
            AliceState::BtcRedeemTransactionPublished { state3 } => {
                Alice::BtcRedeemTransactionPublished {
                    state3: state3.as_ref().clone(),
                }
            }
            AliceState::BtcRedeemed => Alice::Done(AliceEndState::BtcRedeemed),
            AliceState::BtcCancelled {
                beldex_wallet_restore_blockheight,
                transfer_proof,
                state3,
            } => Alice::BtcCancelled {
                beldex_wallet_restore_blockheight,
                transfer_proof,
                state3: state3.as_ref().clone(),
            },
            AliceState::BtcRefunded {
                beldex_wallet_restore_blockheight,
                transfer_proof,
                spend_key,
                state3,
            } => Alice::BtcRefunded {
                beldex_wallet_restore_blockheight,
                transfer_proof,
                spend_key,
                state3: state3.as_ref().clone(),
            },
            AliceState::BtcPunishable {
                beldex_wallet_restore_blockheight,
                transfer_proof,
                state3,
            } => Alice::BtcPunishable {
                beldex_wallet_restore_blockheight,
                transfer_proof,
                state3: state3.as_ref().clone(),
            },
            AliceState::BeldexRefunded => Alice::Done(AliceEndState::BeldexRefunded),
            AliceState::CancelTimelockExpired {
                beldex_wallet_restore_blockheight,
                transfer_proof,
                state3,
            } => Alice::CancelTimelockExpired {
                beldex_wallet_restore_blockheight,
                transfer_proof,
                state3: state3.as_ref().clone(),
            },
            AliceState::BtcPunished { state3 } => Alice::Done(AliceEndState::BtcPunished {
                state3: state3.as_ref().clone(),
            }),
            AliceState::SafelyAborted => Alice::Done(AliceEndState::SafelyAborted),
        }
    }
}

impl From<Alice> for AliceState {
    fn from(db_state: Alice) -> Self {
        match db_state {
            Alice::Started { state3 } => AliceState::Started {
                state3: Box::new(state3),
            },
            Alice::BtcLockTransactionSeen { state3 } => AliceState::BtcLockTransactionSeen {
                state3: Box::new(state3),
            },
            Alice::BtcLocked { state3 } => AliceState::BtcLocked {
                state3: Box::new(state3),
            },
            Alice::BeldexLockTransactionSent {
                beldex_wallet_restore_blockheight,
                transfer_proof,
                state3,
            } => AliceState::BeldexLockTransactionSent {
                beldex_wallet_restore_blockheight,
                transfer_proof,
                state3: Box::new(state3),
            },
            Alice::BeldexLocked {
                beldex_wallet_restore_blockheight,
                transfer_proof,
                state3,
            } => AliceState::BeldexLocked {
                beldex_wallet_restore_blockheight,
                transfer_proof,
                state3: Box::new(state3),
            },
            Alice::BeldexLockTransferProofSent {
                beldex_wallet_restore_blockheight,
                transfer_proof,
                state3,
            } => AliceState::BeldexLockTransferProofSent {
                beldex_wallet_restore_blockheight,
                transfer_proof,
                state3: Box::new(state3),
            },
            Alice::EncSigLearned {
                beldex_wallet_restore_blockheight,
                transfer_proof,
                state3: state,
                encrypted_signature,
            } => AliceState::EncSigLearned {
                beldex_wallet_restore_blockheight,
                transfer_proof,
                state3: Box::new(state),
                encrypted_signature: Box::new(encrypted_signature),
            },
            Alice::BtcRedeemTransactionPublished { state3 } => {
                AliceState::BtcRedeemTransactionPublished {
                    state3: Box::new(state3),
                }
            }
            Alice::CancelTimelockExpired {
                beldex_wallet_restore_blockheight,
                transfer_proof,
                state3,
            } => AliceState::CancelTimelockExpired {
                beldex_wallet_restore_blockheight,
                transfer_proof,
                state3: Box::new(state3),
            },
            Alice::BtcCancelled {
                beldex_wallet_restore_blockheight,
                transfer_proof,
                state3,
            } => AliceState::BtcCancelled {
                beldex_wallet_restore_blockheight,
                transfer_proof,
                state3: Box::new(state3),
            },

            Alice::BtcPunishable {
                beldex_wallet_restore_blockheight,
                transfer_proof,
                state3,
            } => AliceState::BtcPunishable {
                beldex_wallet_restore_blockheight,
                transfer_proof,
                state3: Box::new(state3),
            },
            Alice::BtcRefunded {
                beldex_wallet_restore_blockheight,
                transfer_proof,
                spend_key,
                state3,
            } => AliceState::BtcRefunded {
                beldex_wallet_restore_blockheight,
                transfer_proof,
                spend_key,
                state3: Box::new(state3),
            },
            Alice::Done(end_state) => match end_state {
                AliceEndState::SafelyAborted => AliceState::SafelyAborted,
                AliceEndState::BtcRedeemed => AliceState::BtcRedeemed,
                AliceEndState::BeldexRefunded => AliceState::BeldexRefunded,
                AliceEndState::BtcPunished { state3 } => AliceState::BtcPunished {
                    state3: Box::new(state3),
                },
            },
        }
    }
}

impl fmt::Display for Alice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Alice::Started { .. } => write!(f, "Started"),
            Alice::BtcLockTransactionSeen { .. } => {
                write!(f, "Bitcoin lock transaction in mempool")
            }
            Alice::BtcLocked { .. } => f.write_str("Bitcoin locked"),
            Alice::BeldexLockTransactionSent { .. } => f.write_str("Beldex lock transaction sent"),
            Alice::BeldexLocked { .. } => f.write_str("Beldex locked"),
            Alice::BeldexLockTransferProofSent { .. } => {
                f.write_str("Beldex lock transfer proof sent")
            }
            Alice::EncSigLearned { .. } => f.write_str("Encrypted signature learned"),
            Alice::BtcRedeemTransactionPublished { .. } => {
                f.write_str("Bitcoin redeem transaction published")
            }
            Alice::CancelTimelockExpired { .. } => f.write_str("Cancel timelock is expired"),
            Alice::BtcCancelled { .. } => f.write_str("Bitcoin cancel transaction published"),
            Alice::BtcPunishable { .. } => f.write_str("Bitcoin punishable"),
            Alice::BtcRefunded { .. } => f.write_str("Beldex refundable"),
            Alice::Done(end_state) => write!(f, "Done: {}", end_state),
        }
    }
}
