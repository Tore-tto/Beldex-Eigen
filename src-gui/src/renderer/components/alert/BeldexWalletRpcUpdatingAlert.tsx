import { Box, LinearProgress } from "@material-ui/core";
import { Alert } from "@material-ui/lab";
import { useAppSelector } from "../../../store/hooks";
import { RootState } from "../../store/storeRenderer";

export default function BeldexWalletRpcUpdatingAlert() {
  const updateState = useAppSelector(
    (s: RootState) => s.rpc.state.beldexWalletRpc.updateState,
  );

  if (updateState === false || !updateState.progress) {
    return null;
  }

  const progress = Number.parseFloat(
    updateState.progress.substring(0, updateState.progress.length - 1),
  );

  return (
    <Alert severity="info">
      <Box style={{ display: "flex", flexDirection: "column", gap: "0.5rem" }}>
        <span>The Beldex wallet is updating. This may take a few moments</span>
        <LinearProgress
          variant="determinate"
          value={progress}
          title="Download progress"
        />
      </Box>
    </Alert>
  );
}
