export function satsToBtc(sats: number): number {
  return sats / 100000000;
}

export function btcToSats(btc: number): number {
  return btc * 100000000;
}

export function piconerosToBeldex(piconeros: number): number {
  return piconeros / 1000000000000;
}

export function isBeldexAddressValid(address: string, testnet: boolean) {
  const re = testnet
    ? "[9AL][1-9A-HJ-NP-Za-km-z]{94,105}"
    : "b[1-9A-HJ-NP-Za-km-z]{96,106}";
  return new RegExp(`(?:^${re}$)`).test(address);
}

export function isBtcAddressValid(address: string, testnet: boolean) {
  const re = testnet
    ? "(tb1|[mn2])[a-zA-HJ-NP-Z0-9]{25,62}"
    : "(bc1|[13])[a-zA-HJ-NP-Z0-9]{25,62}";
  return new RegExp(`(?:^${re}$)`).test(address);
}

export function getBitcoinTxExplorerUrl(txid: string, testnet: boolean) {
  return `https://blockchair.com/bitcoin${testnet ? "/testnet" : ""
    }/transaction/${txid}`;
}

export function getBeldexTxExplorerUrl(txid: string, testnet: boolean) {
  if (testnet) {
    return `http://154.26.139.105/tx/${txid}`;
  }
  return `https://explorer.beldex.io/tx/${txid}`;
}

export function secondsToDays(seconds: number): number {
  return seconds / 86400;
}
