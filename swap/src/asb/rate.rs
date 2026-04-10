use crate::{bitcoin, beldex};
use anyhow::{Context, Result};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use std::fmt::{Debug, Display, Formatter};

/// Represents the rate at which we are willing to trade 1 BDX.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rate {
    /// Represents the asking price from the market.
    ask: bitcoin::Amount,
    /// The spread which should be applied to the market asking price.
    ask_spread: Decimal,
}

const ZERO_SPREAD: Decimal = Decimal::from_parts(0, 0, 0, false, 0);

impl Rate {
    pub const ZERO: Rate = Rate {
        ask: bitcoin::Amount::ZERO,
        ask_spread: ZERO_SPREAD,
    };

    pub fn new(ask: bitcoin::Amount, ask_spread: Decimal) -> Self {
        Self { ask, ask_spread }
    }

    /// Computes the asking price at which we are willing to sell 1 BDX.
    ///
    /// This applies the spread to the market asking price.
    pub fn ask(&self) -> Result<bitcoin::Amount> {
        let sats = self.ask.to_sat();
        let sats = Decimal::from(sats);

        let additional_sats = sats * self.ask_spread;
        let additional_sats = bitcoin::Amount::from_sat(
            additional_sats
                .to_u64()
                .context("Failed to fit spread into u64")?,
        );

        Ok(self.ask + additional_sats)
    }

    /// Calculate a sell quote for a given BTC amount.
    pub fn sell_quote(&self, quote: bitcoin::Amount) -> Result<beldex::Amount> {
        Self::quote(self.ask()?, quote)
    }

    fn quote(rate: bitcoin::Amount, quote: bitcoin::Amount) -> Result<beldex::Amount> {
        // quote (btc) = rate * base (bdx)
        // base = quote / rate

        let quote_in_sats = quote.to_sat();
        let quote_in_btc = Decimal::from(quote_in_sats)
            .checked_div(Decimal::from(bitcoin::Amount::ONE_BTC.to_sat()))
            .context("Division overflow")?;

        let rate_in_btc = Decimal::from(rate.to_sat())
            .checked_div(Decimal::from(bitcoin::Amount::ONE_BTC.to_sat()))
            .context("Division overflow")?;

        let base_in_bdx = quote_in_btc
            .checked_div(rate_in_btc)
            .context("Division overflow")?;
        let base_in_atomic = base_in_bdx * Decimal::from(beldex::Amount::ONE_BDX.as_atomic());

        let base_in_atomic = base_in_atomic
            .to_u64()
            .context("Failed to fit atomic amount into a u64")?;

        Ok(beldex::Amount::from_atomic(base_in_atomic))
    }
}

impl Display for Rate {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.ask)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TWO_PERCENT: Decimal = Decimal::from_parts(2, 0, 0, false, 2);
    const ONE: Decimal = Decimal::from_parts(1, 0, 0, false, 0);

    #[test]
    fn sell_quote() {
        let asking_price = bitcoin::Amount::from_btc(0.002_500).unwrap();
        let rate = Rate::new(asking_price, ZERO_SPREAD);

        let btc_amount = bitcoin::Amount::from_btc(2.5).unwrap();

        let bdx_amount = rate.sell_quote(btc_amount).unwrap();

        assert_eq!(bdx_amount, beldex::Amount::from_beldex(1000.0).unwrap())
    }

    #[test]
    fn applies_spread_to_asking_price() {
        let asking_price = bitcoin::Amount::from_sat(100);
        let rate = Rate::new(asking_price, TWO_PERCENT);

        let amount = rate.ask().unwrap();

        assert_eq!(amount.to_sat(), 102);
    }

    #[test]
    fn given_spread_of_two_percent_when_caluclating_sell_quote_factor_between_should_be_two_percent(
    ) {
        let asking_price = bitcoin::Amount::from_btc(0.004).unwrap();

        let rate_no_spread = Rate::new(asking_price, ZERO_SPREAD);
        let rate_with_spread = Rate::new(asking_price, TWO_PERCENT);

        let bdx_no_spread = rate_no_spread.sell_quote(bitcoin::Amount::ONE_BTC).unwrap();
        let bdx_with_spread = rate_with_spread
            .sell_quote(bitcoin::Amount::ONE_BTC)
            .unwrap();

        let bdx_factor =
            bdx_no_spread.as_atomic_decimal() / bdx_with_spread.as_atomic_decimal() - ONE;

        assert!(bdx_with_spread < bdx_no_spread);
        assert_eq!(bdx_factor.round_dp(8), TWO_PERCENT); // round to 8 decimal
                                                         // places to show that
                                                         // it is really close
                                                         // to two percent
    }
}
