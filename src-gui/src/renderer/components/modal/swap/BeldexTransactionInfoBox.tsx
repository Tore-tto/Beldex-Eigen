import { ReactNode } from "react";
import BeldexIcon from "renderer/components/icons/BeldexIcon";
import { isTestnet } from "store/config";
import { getBeldexTxExplorerUrl } from "utils/conversionUtils";
import TransactionInfoBox from "./TransactionInfoBox";

type Props = {
  title: string;
  txId: string;
  additionalContent: ReactNode;
  loading: boolean;
};

export default function BeldexTransactionInfoBox({ txId, ...props }: Props) {
  const explorerUrl = getBeldexTxExplorerUrl(txId, isTestnet());

  return (
    <TransactionInfoBox
      txId={txId}
      explorerUrl={explorerUrl}
      icon={<BeldexIcon />}
      {...props}
    />
  );
}
