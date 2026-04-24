import { Tooltip } from "@material-ui/core";
import { useAppSelector } from "store/hooks";
import { piconerosToBeldex, satsToBtc } from "utils/conversionUtils";

type Amount = number | null | undefined;

export function AmountWithUnit({
  amount,
  unit,
  fixedPrecision,
  dollarRate,
}: {
  amount: Amount;
  unit: string;
  fixedPrecision: number;
  dollarRate?: Amount;
}) {
  return (
    <Tooltip
      arrow
      title={
        dollarRate != null && amount != null
          ? `≈ $${(dollarRate * amount).toFixed(2)}`
          : ""
      }
    >
      <span>
        {amount != null
          ? Number.parseFloat(amount.toFixed(fixedPrecision))
          : "?"}{" "}
        {unit}
      </span>
    </Tooltip>
  );
}

AmountWithUnit.defaultProps = {
  dollarRate: null,
};

export function BitcoinAmount({ amount }: { amount: Amount }) {
  const btcUsdRate = useAppSelector((state) => state.rates.btcPrice);

  return (
    <AmountWithUnit
      amount={amount}
      unit="BTC"
      fixedPrecision={6}
      dollarRate={btcUsdRate}
    />
  );
}

export function BeldexAmount({ amount }: { amount: Amount }) {
  const bdxUsdRate = useAppSelector((state) => state.rates.bdxPrice);

  return (
    <AmountWithUnit
      amount={amount}
      unit="BDX"
      fixedPrecision={4}
      dollarRate={bdxUsdRate}
    />
  );
}

export function BeldexBitcoinExchangeRate(
  state: { rate: Amount } | { satsAmount: number; piconerosAmount: number },
) {
  if ("rate" in state) {
    return (
      <AmountWithUnit amount={state.rate} unit="BTC/BDX" fixedPrecision={8} />
    );
  }

  const rate =
    satsToBtc(state.satsAmount) / piconerosToBeldex(state.piconerosAmount);

  return <AmountWithUnit amount={rate} unit="BTC/BDX" fixedPrecision={8} />;
}

export function BeldexSatsExchangeRate({ rate }: { rate: Amount }) {
  const btc = satsToBtc(rate);

  return <AmountWithUnit amount={btc} unit="BTC/BDX" fixedPrecision={6} />;
}

export function SatsAmount({ amount }: { amount: Amount }) {
  const btcAmount = amount == null ? null : satsToBtc(amount);
  return <BitcoinAmount amount={btcAmount} />;
}

export function BeldexUnitsAmount({ amount }: { amount: Amount }) {
  return (
    <BeldexAmount
      amount={amount == null ? null : piconerosToBeldex(amount)}
    />
  );
}
