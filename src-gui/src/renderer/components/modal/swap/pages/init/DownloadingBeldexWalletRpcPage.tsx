import { BeldexWalletRpcUpdateState } from "../../../../../../models/storeModel";
import CircularProgressWithSubtitle from "../../CircularProgressWithSubtitle";

export default function DownloadingBeldexWalletRpcPage({
  updateState,
}: {
  updateState: BeldexWalletRpcUpdateState;
}) {
  return (
    <CircularProgressWithSubtitle
      description={`Updating beldex-wallet-rpc (${updateState.progress}) `}
    />
  );
}
