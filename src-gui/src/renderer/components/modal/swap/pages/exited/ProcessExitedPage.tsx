import { Box, Typography } from "@material-ui/core";
import { TauriSwapProgressEvent } from "models/tauriModel";
import SwapStatePage from "../SwapStatePage";
import SwapLogs from "./SwapLogs";

export default function ProcessExitedPage({
  prevState,
  swapId,
}: {
  prevState: TauriSwapProgressEvent | null;
  swapId: string;
}) {
  // If we have a previous state, we can show the user the last state of the swap
  // We only show the last state if its a final state (BeldexRedeemInMempool, BtcRefunded, BtcPunished)
  if (
    prevState != null &&
    (prevState.type === "BeldexRedeemInMempool" ||
      prevState.type === "BtcRefunded" ||
      prevState.type === "BtcPunished")
  ) {
    return (
      <SwapStatePage
        state={{
          curr: prevState,
          prev: null,
          swapId,
        }}
      />
    );
  }

  return (
    <Box>
      <Typography variant="body1" gutterBottom>
        The swap process has exited, but it did not reach a final state. This
        could be due to a network error, manual cancellation, or the swap setup
        not being completed.
      </Typography>
      <Typography variant="body2" color="textSecondary" gutterBottom>
        You can try to resume the swap from the transaction history if possible,
        or check the logs below for more details.
      </Typography>
      <SwapLogs swapId={swapId} />
    </Box>
  );
}
