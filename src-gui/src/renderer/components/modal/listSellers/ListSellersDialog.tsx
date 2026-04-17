import {
  Box,
  Button,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
  DialogTitle,
  makeStyles,
  TextField,
  Theme,
} from "@material-ui/core";
import { Multiaddr } from "multiaddr";
import { useSnackbar } from "notistack";
import { ChangeEvent, useState } from "react";
import { useDispatch } from "react-redux";
import PromiseInvokeButton from "renderer/components/PromiseInvokeButton";
import { listSellers } from "renderer/rpc";
import { discoveredProvidersByRendezvous } from "store/features/providersSlice";
import { isTestnet } from "store/config";
import { Seller } from "models/tauriModel";
import { ProviderStatus } from "models/apiModel";

const PRESET_RENDEZVOUS_POINTS = [
  "/ip4/194.5.152.31/tcp/9939/p2p/12D3KooWNQGN39V6je7sgQBEjuKEMMNEeA5fPUbX4YMm2nrGocpF",
  "/dns4/rendezvous.beldex.io/tcp/8888/p2p/12D3KooWPD4uHN74SHotLN7VCH7Fm8zZgaNVymYcpeF1fpD2guc9",
];

const useStyles = makeStyles((theme: Theme) => ({
  chipOuter: {
    display: "flex",
    flexWrap: "wrap",
    gap: theme.spacing(1),
  },
}));

type ListSellersDialogProps = {
  open: boolean;
  onClose: () => void;
};

export default function ListSellersDialog({
  open,
  onClose,
}: ListSellersDialogProps) {
  const classes = useStyles();
  const dispatch = useDispatch();
  const [rendezvousAddress, setRendezvousAddress] = useState("");
  const { enqueueSnackbar } = useSnackbar();

  function handleMultiAddrChange(event: ChangeEvent<HTMLInputElement>) {
    setRendezvousAddress(event.target.value);
  }

  function getMultiAddressError(): string | null {
    try {
      const multiAddress = new Multiaddr(rendezvousAddress);
      if (!multiAddress.protoNames().includes("p2p")) {
        return "The multi address must contain the peer id (/p2p/)";
      }
      return null;
    } catch {
      return "Not a valid multi address";
    }
  }

  function handleSuccess(sellers: Seller[]) {
    const providers: ProviderStatus[] = sellers
      .filter((seller: any) => seller.status.type === "Online")
      .map((seller: any) => {
        const quote = seller.status.content;
        const multiaddr = new Multiaddr(seller.multiaddr);
        const peerId = multiaddr.getPeerId();

        return {
          multiAddr: multiaddr.decapsulate("/p2p/" + peerId).toString(),
          peerId: peerId!,
          testnet: isTestnet(),
          price: quote.price,
          minSwapAmount: quote.min_quantity,
          maxSwapAmount: quote.max_quantity,
        };
      });

    dispatch(discoveredProvidersByRendezvous(providers));

    let message: string;
    const amountOfSellers = sellers.length;

    switch (amountOfSellers) {
      case 0:
        message = `No providers were discovered at the rendezvous point`;
        break;
      case 1:
        message = `Discovered one provider at the rendezvous point`;
        break;
      default:
        message = `Discovered ${amountOfSellers} providers at the rendezvous point`;
    }

    enqueueSnackbar(message, {
      variant: amountOfSellers > 0 ? "success" : "warning",
      autoHideDuration: 5000,
    });

    onClose();
  }

  return (
    <Dialog onClose={onClose} open={open}>
      <DialogTitle>Discover swap providers</DialogTitle>
      <DialogContent dividers>
        <DialogContentText>
          The rendezvous protocol provides a way to discover providers (trading
          partners) without relying on one singular centralized institution. By
          manually connecting to a rendezvous point run by a volunteer, you can
          discover providers and then connect and swap with them.
        </DialogContentText>
        <TextField
          autoFocus
          margin="dense"
          label="Rendezvous point"
          fullWidth
          helperText={
            getMultiAddressError() || "Multiaddress of the rendezvous point"
          }
          value={rendezvousAddress}
          onChange={handleMultiAddrChange}
          placeholder="/ip4/127.0.0.1/tcp/8888/p2p/12D3KooWPD4uHN74SHotLN7VCH7Fm8zZgaNVymYcpeF1fpD2guc9"
          error={!!getMultiAddressError()}
        />
        <Box className={classes.chipOuter}>
          {PRESET_RENDEZVOUS_POINTS.map((rAddress) => (
            <Chip
              key={rAddress}
              clickable
              label={`${rAddress.substring(
                0,
                Math.min(rAddress.length - 1, 20),
              )}...`}
              onClick={() => setRendezvousAddress(rAddress)}
            />
          ))}
        </Box>
      </DialogContent>
      <DialogActions>
        <Button onClick={onClose}>Cancel</Button>
        <PromiseInvokeButton
          variant="contained"
          disabled={!(rendezvousAddress && !getMultiAddressError())}
          color="primary"
          onSuccess={handleSuccess}
          displayErrorSnackbar
          onInvoke={async () => {
            return await listSellers(rendezvousAddress);
          }}
        >
          Connect
        </PromiseInvokeButton>
      </DialogActions>
    </Dialog>
  );
}
