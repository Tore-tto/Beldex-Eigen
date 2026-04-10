import { Box, DialogContentText } from "@material-ui/core";
import { TauriSwapProgressEventContent } from "models/tauriModelExt";
import BeldexTransactionInfoBox from "../../BeldexTransactionInfoBox";

export default function BeldexLockTxInMempoolPage({
  bdx_lock_tx_confirmations,
  bdx_lock_txid,
}: TauriSwapProgressEventContent<"BeldexLockTxInMempool">) {
  const additionalContent = `Confirmations: ${bdx_lock_tx_confirmations}/10`;

  return (
    <Box>
      <DialogContentText>
        They have published their Beldex lock transaction. The swap will proceed
        once the transaction has been confirmed.
      </DialogContentText>

      <BeldexTransactionInfoBox
        title="Beldex Lock Transaction"
        txId={bdx_lock_txid}
        additionalContent={additionalContent}
        loading
      />
    </Box>
  );
}
