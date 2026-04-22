import { invoke as invokeUnsafe } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  BalanceArgs,
  BalanceResponse,
  BuyBeldexArgs,
  BuyBeldexResponse,
  GetSwapInfoResponse,
  BeldexRecoveryArgs,
  ResumeSwapArgs,
  ResumeSwapResponse,
  SuspendCurrentSwapResponse,
  TauriContextStatusEvent,
  TauriSwapProgressEventWrapper,
  WithdrawBtcArgs,
  WithdrawBtcResponse,
  GetLogsArgs,
  GetLogsResponse,
  Seller,
  ListSellersArgs,
  StartDaemonArgs,
} from "models/tauriModel";
import {
  contextStatusEventReceived,
  rpcSetBalance,
  rpcSetSwapInfo,
} from "store/features/rpcSlice";
import { swapTauriEventReceived } from "store/features/swapSlice";
import { store } from "./store/storeRenderer";
import { Provider } from "models/apiModel";
import { providerToConcatenatedMultiAddr } from "utils/multiAddrUtils";
import { BeldexRecoveryResponse } from "models/rpcModel";

export async function initEventListeners() {
  // This operation is in-expensive
  // We do this in case we miss the context init progress event because the frontend took too long to load
  // TOOD: Replace this with a more reliable mechanism (such as an event replay mechanism)
  if (await checkContextAvailability()) {
    store.dispatch(contextStatusEventReceived({ type: "Available" }));
  }

  listen<TauriSwapProgressEventWrapper>("swap-progress-update", (event) => {
    console.log("Received swap progress event", event.payload);
    store.dispatch(swapTauriEventReceived(event.payload));
  });

  listen<TauriContextStatusEvent>("context-init-progress-update", (event) => {
    console.log("Received context init progress event", event.payload);
    store.dispatch(contextStatusEventReceived(event.payload));
  });
}

async function invoke<ARGS, RESPONSE>(
  command: string,
  args: ARGS,
): Promise<RESPONSE> {
  return invokeUnsafe(command, {
    args: args as Record<string, unknown>,
  }) as Promise<RESPONSE>;
}

async function invokeNoArgs<RESPONSE>(command: string): Promise<RESPONSE> {
  return invokeUnsafe(command) as Promise<RESPONSE>;
}

export async function checkBitcoinBalance() {
  const response = await invoke<BalanceArgs, BalanceResponse>("get_balance", {
    force_refresh: true,
  });

  store.dispatch(rpcSetBalance(response.balance));
}

export async function getAllSwapInfos() {
  try {
    const response =
      await invokeNoArgs<GetSwapInfoResponse[]>("get_swap_infos_all");

    response.forEach((swapInfo) => {
      store.dispatch(rpcSetSwapInfo(swapInfo));
    });
  } catch (e) {
    console.error("Failed to get all swap infos", e);
  }
}

export async function withdrawBtc(address: string): Promise<string> {
  const response = await invoke<WithdrawBtcArgs, WithdrawBtcResponse>(
    "withdraw_btc",
    {
      address,
      amount: null,
    },
  );

  return response.txid;
}

export async function buyBeldex(
  seller: Provider,
  bitcoin_change_address: string,
  beldex_receive_address: string,
) {
  await invoke<BuyBeldexArgs, BuyBeldexResponse>("buy_bdx", {
    seller: providerToConcatenatedMultiAddr(seller),
    bitcoin_change_address,
    beldex_receive_address,
  });
}

export async function resumeSwap(swapId: string) {
  await invoke<ResumeSwapArgs, ResumeSwapResponse>("resume_swap", {
    swap_id: swapId,
  });
}

export async function suspendCurrentSwap() {
  await invokeNoArgs<SuspendCurrentSwapResponse>("suspend_current_swap");
}

export async function getBeldexRecoveryKeys(
  swapId: string,
): Promise<BeldexRecoveryResponse> {
  return await invoke<BeldexRecoveryArgs, BeldexRecoveryResponse>(
    "beldex_recovery",
    {
      swap_id: swapId,
    },
  );
}

export async function checkContextAvailability(): Promise<boolean> {
  const available = await invokeNoArgs<boolean>("is_context_available");
  return available;
}

export async function getLogs(swapId: string): Promise<string[]> {
  const response = await invoke<GetLogsArgs, GetLogsResponse>("get_logs", {
    swap_id: swapId,
    redact: true,
    logs_dir: null,
  });

  return response.logs;
}

export async function startDaemon() {
  await invokeNoArgs("start_daemon");
}

export async function isDaemonRunning(): Promise<boolean> {
  return await invokeNoArgs<boolean>("is_daemon_running");
}

export async function listSellers(rendezvousPoint: string): Promise<Seller[]> {
  const response = await invoke<ListSellersArgs, any>("list_sellers", {
    rendezvous_point: rendezvousPoint,
  });

  return response.sellers;
}
