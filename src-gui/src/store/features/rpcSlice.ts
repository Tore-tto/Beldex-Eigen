import { createSlice, PayloadAction } from "@reduxjs/toolkit";
import { ExtendedProviderStatus, ProviderStatus } from "models/apiModel";
import {
  GetSwapInfoResponse,
  TauriContextStatusEvent,
} from "models/tauriModel";
import { BeldexRecoveryResponse } from "../../models/rpcModel";
import { GetSwapInfoResponseExt } from "models/tauriModelExt";

interface State {
  balance: number | null;
  withdrawTxId: string | null;
  rendezvous_discovered_sellers: (ExtendedProviderStatus | ProviderStatus)[];
  swapInfos: {
    [swapId: string]: GetSwapInfoResponseExt;
  };
  beldexRecovery: {
    swapId: string;
    keys: BeldexRecoveryResponse;
  } | null;
  beldexWallet: {
    isSyncing: boolean;
  };
  beldexWalletRpc: {
    // TODO: Reimplement this using Tauri
    updateState: false | { progress: string };
  };
}

export interface RPCSlice {
  status: TauriContextStatusEvent | null;
  state: State;
  busyEndpoints: string[];
}

const initialState: RPCSlice = {
  status: null,
  state: {
    balance: null,
    withdrawTxId: null,
    rendezvous_discovered_sellers: [],
    swapInfos: {},
    beldexRecovery: null,
    beldexWallet: {
      isSyncing: false,
    },
    beldexWalletRpc: {
      updateState: false,
    },
  },
  busyEndpoints: [],
};

export const rpcSlice = createSlice({
  name: "rpc",
  initialState,
  reducers: {
    contextStatusEventReceived(
      slice,
      action: PayloadAction<TauriContextStatusEvent>,
    ) {
      slice.status = action.payload;
    },
    rpcSetBalance(slice, action: PayloadAction<number>) {
      slice.state.balance = action.payload;
    },
    rpcSetWithdrawTxId(slice, action: PayloadAction<string>) {
      slice.state.withdrawTxId = action.payload;
    },
    rpcSetRendezvousDiscoveredProviders(
      slice,
      action: PayloadAction<(ExtendedProviderStatus | ProviderStatus)[]>,
    ) {
      slice.state.rendezvous_discovered_sellers = action.payload;
    },
    rpcResetWithdrawTxId(slice) {
      slice.state.withdrawTxId = null;
    },
    rpcSetSwapInfo(slice, action: PayloadAction<GetSwapInfoResponse>) {
      slice.state.swapInfos[action.payload.swap_id] =
        action.payload as GetSwapInfoResponseExt;
    },
    rpcSetEndpointBusy(slice, action: PayloadAction<string>) {
      if (!slice.busyEndpoints.includes(action.payload)) {
        slice.busyEndpoints.push(action.payload);
      }
    },
    rpcSetEndpointFree(slice, action: PayloadAction<string>) {
      const index = slice.busyEndpoints.indexOf(action.payload);
      if (index >= 0) {
        slice.busyEndpoints.splice(index);
      }
    },
    rpcSetBeldexRecoveryKeys(
      slice,
      action: PayloadAction<[string, BeldexRecoveryResponse]>,
    ) {
      const swapId = action.payload[0];
      const keys = action.payload[1];

      slice.state.beldexRecovery = {
        swapId,
        keys,
      };
    },
    rpcResetBeldexRecoveryKeys(slice) {
      slice.state.beldexRecovery = null;
    },
  },
});

export const {
  contextStatusEventReceived,
  rpcSetBalance,
  rpcSetWithdrawTxId,
  rpcResetWithdrawTxId,
  rpcSetEndpointBusy,
  rpcSetEndpointFree,
  rpcSetRendezvousDiscoveredProviders,
  rpcSetSwapInfo,
  rpcSetBeldexRecoveryKeys,
  rpcResetBeldexRecoveryKeys,
} = rpcSlice.actions;

export default rpcSlice.reducer;
