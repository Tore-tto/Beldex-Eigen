import { Typography } from "@material-ui/core";
import BeldexIcon from "../../icons/BeldexIcon";
import DepositAddressInfoBox from "../../modal/swap/DepositAddressInfoBox";

const BDX_DONATE_ADDRESS =
  "bxcg8cczftvBbTiXmEWCBLLfFqcyxcEm8556XfGnxthPhta9rthJUKYDzedx4ZjtPviMHqZ9UxedeSc3B6ThE57f1BnTSQV3c";

export default function DonateInfoBox() {
  return (
    <DepositAddressInfoBox
      title="Donate"
      address={BDX_DONATE_ADDRESS}
      icon={<BeldexIcon />}
      additionalContent={
        <Typography variant="subtitle2">
          We rely on generous donors like you to keep development moving
          forward. To bring Atomic Swaps to life, we need resources. If you have
          the possibility, please consider making a donation to the project. All
          funds will be used to support contributors and critical
          infrastructure.
        </Typography>
      }
    />
  );
}
