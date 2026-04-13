import {
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
  DialogTitle,
  TextField,
} from "@material-ui/core";
import { Multiaddr } from "multiaddr";
import { ChangeEvent, useState } from "react";
import { manualProviderAdded } from "store/features/providersSlice";
import { useAppDispatch } from "store/hooks";
import { isTestnet } from "store/config";

type ProviderSubmitDialogProps = {
  open: boolean;
  onClose: () => void;
};

export default function ProviderSubmitDialog({
  open,
  onClose,
}: ProviderSubmitDialogProps) {
  const [multiAddr, setMultiAddr] = useState("");
  const [peerId, setPeerId] = useState("");
  const dispatch = useAppDispatch();

  async function handleProviderSubmit() {
    if (multiAddr && peerId) {
      // We still try to submit to the registry, but we don't wait for it
      fetch("https://api.unstoppableswap.net/api/submit-provider", {
        method: "post",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          multiAddr,
          peerId,
        }),
      }).catch((e) => console.error("Failed to submit to registry", e));

      // Construct the full multiaddress if it doesn't already contain the peer id
      let fullMultiAddr = multiAddr;
      try {
        const ma = new Multiaddr(multiAddr);
        if (!ma.protoNames().includes("p2p")) {
          fullMultiAddr = `${
            multiAddr.endsWith("/") ? multiAddr.slice(0, -1) : multiAddr
          }/p2p/${peerId}`;
        }
      } catch (e) {
        // Fallback if parsing fails (though validation should have caught it)
        fullMultiAddr = `${
          multiAddr.endsWith("/") ? multiAddr.slice(0, -1) : multiAddr
        }/p2p/${peerId}`;
      }

      dispatch(
        manualProviderAdded({
          multiAddr: fullMultiAddr,
          peerId,
          testnet: isTestnet(),
          price: 100, // 0.000001 BTC per BDX (placeholder)
          minSwapAmount: 100000, // 0.001 BTC (placeholder)
          maxSwapAmount: 10000000, // 0.1 BTC (placeholder)
        }),
      );

      setMultiAddr("");
      setPeerId("");
      onClose();
    }
  }

  function handleMultiAddrChange(event: ChangeEvent<HTMLInputElement>) {
    setMultiAddr(event.target.value);
  }

  function handlePeerIdChange(event: ChangeEvent<HTMLInputElement>) {
    setPeerId(event.target.value);
  }

  function getMultiAddressError(): string | null {
    try {
      const multiAddress = new Multiaddr(multiAddr);
      if (multiAddress.protoNames().includes("p2p")) {
        return "The multi address should not contain the peer id (/p2p/)";
      }
      if (multiAddress.protoNames().find((name) => name.includes("onion"))) {
        return "It is currently not possible to add a provider that is only reachable via Tor";
      }
      return null;
    } catch (e) {
      return "Not a valid multi address";
    }
  }

  return (
    <Dialog onClose={onClose} open={open}>
      <DialogTitle>Submit a provider to the public registry</DialogTitle>
      <DialogContent dividers>
        <DialogContentText>
          If the provider is valid and reachable, it will be displayed to all
          other users to trade with.
        </DialogContentText>
        <TextField
          autoFocus
          margin="dense"
          label="Multiaddress"
          fullWidth
          helperText={
            getMultiAddressError() ||
            "Tells the swap client where the provider can be reached"
          }
          value={multiAddr}
          onChange={handleMultiAddrChange}
          placeholder="/ip4/182.3.21.93/tcp/9939"
          error={!!getMultiAddressError()}
        />
        <TextField
          margin="dense"
          label="Peer ID"
          fullWidth
          helperText="Identifies the provider and allows for secure communication"
          value={peerId}
          onChange={handlePeerIdChange}
          placeholder="12D3KooWCdMKjesXMJz1SiZ7HgotrxuqhQJbP5sgBm2BwP1cqThi"
        />
      </DialogContent>
      <DialogActions>
        <Button onClick={onClose}>Cancel</Button>
        <Button
          variant="contained"
          onClick={handleProviderSubmit}
          disabled={!(multiAddr && peerId && !getMultiAddressError())}
          color="primary"
        >
          Submit
        </Button>
      </DialogActions>
    </Dialog>
  );
}
