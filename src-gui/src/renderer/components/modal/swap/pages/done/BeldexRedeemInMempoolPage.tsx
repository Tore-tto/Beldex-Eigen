import { Box, DialogContentText } from "@material-ui/core";
import { TauriSwapProgressEventContent } from "models/tauriModelExt";
import FeedbackInfoBox from "../../../../pages/help/FeedbackInfoBox";
import BeldexTransactionInfoBox from "../../BeldexTransactionInfoBox";

export default function BeldexRedeemInMempoolPage({
  bdx_redeem_address,
  bdx_redeem_txid,
}: TauriSwapProgressEventContent<"BeldexRedeemInMempool">) {
  // TODO: Reimplement this using Tauri
  //const additionalContent = swap
  //  ? `This transaction transfers ${getSwapBeldexAmount(swap).toFixed(6)} BDX to ${
  //      state?.bobBeldexRedeemAddress
  //    }`
  //  : null;

  return (
    <Box>
      <DialogContentText>
        The swap was successful and the Beldex has been sent to the address you
        specified. The swap is completed and you may exit the application now.
      </DialogContentText>
      <Box
        style={{
          display: "flex",
          flexDirection: "column",
          gap: "0.5rem",
        }}
      >
        <BeldexTransactionInfoBox
          title="Beldex Redeem Transaction"
          txId={bdx_redeem_txid}
          additionalContent={`The funds have been sent to the address ${bdx_redeem_address}`}
          loading={false}
        />
        <FeedbackInfoBox />
      </Box>
    </Box>
  );
}
