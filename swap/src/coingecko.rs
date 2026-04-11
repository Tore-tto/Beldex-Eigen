use anyhow::{anyhow, Context, Result};
use bitcoin::Amount;
use serde::Deserialize;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;

const COINGECKO_BDX_BTC_URL: &str =
    "https://api.coingecko.com/api/v3/simple/price?ids=beldex&vs_currencies=btc";

/// Poll CoinGecko for BDX/BTC price updates every `interval`.
///
/// If a request fails, it will be retried with exponential backoff.
/// The structure mirrors `kraken::connect()` so it can be used as a drop-in.
pub fn connect(interval: Duration) -> Result<PriceUpdates> {
    let (price_update, price_update_receiver) = watch::channel(Err(Error::NotYetAvailable));
    let price_update = Arc::new(price_update);

    tokio::spawn(async move {
        let backoff = backoff::ExponentialBackoff {
            max_elapsed_time: None,
            ..backoff::ExponentialBackoff::default()
        };

        let result = backoff::future::retry_notify::<Infallible, _, _, _, _, _>(
            backoff,
            || {
                let price_update = price_update.clone();
                async move {
                    loop {
                        match fetch_beldex_btc_price().await {
                            Ok(ask) => {
                                tracing::info!("Fetched BDX/BTC price: {} BTC per BDX", ask);
                                let send_result = price_update.send(Ok(wire::PriceUpdate { ask }));
                                if send_result.is_err() {
                                    return Err(backoff::Error::Permanent(anyhow!(
                                        "receiver disconnected"
                                    )));
                                }
                            }
                            Err(e) => {
                                tracing::warn!("Failed to fetch BDX/BTC price: {:#}", e);
                                return Err(backoff::Error::Transient { err: e, retry_after: None });
                            }
                        }

                        tokio::time::sleep(interval).await;
                    }
                }
            },
            |error, next: Duration| {
                tracing::info!(
                    %error,
                    "CoinGecko BDX/BTC fetch failed, retrying in {}ms",
                    next.as_millis()
                );
            },
        )
        .await;

        match result {
            Err(e) => {
                tracing::warn!("CoinGecko rate updates failed permanently: {:#}", e);
                let _ = price_update.send(Err(Error::PermanentFailure));
            }
            Ok(never) => match never {},
        }
    });

    Ok(PriceUpdates {
        inner: price_update_receiver,
    })
}

async fn fetch_beldex_btc_price() -> Result<Amount> {
    let client = reqwest::Client::builder()
        .user_agent("curl/8.5.0")
        .build()
        .context("Failed to build HTTP client")?;

    let response = client
        .get(COINGECKO_BDX_BTC_URL)
        .send()
        .await
        .context("HTTP request to CoinGecko failed")?
        .json::<CoinGeckoResponse>()
        .await
        .context("Failed to deserialize CoinGecko response")?;

    let btc_f64 = response.beldex.btc;
    let amount = Amount::from_btc(btc_f64).context("Failed to convert BDX/BTC price to Amount")?;

    Ok(amount)
}

// --- Wire types ---------------------------------------------------------

/// Top-level response: `{ "beldex": { "btc": 0.00000041 } }`
#[derive(Debug, Deserialize)]
struct CoinGeckoResponse {
    beldex: BeldexPrice,
}

#[derive(Debug, Deserialize)]
struct BeldexPrice {
    btc: f64,
}

// --- Public types (mirrors kraken module) --------------------------------

#[derive(Clone, Debug)]
pub struct PriceUpdates {
    inner: watch::Receiver<PriceUpdate>,
}

impl PriceUpdates {
    pub async fn wait_for_next_update(&mut self) -> Result<PriceUpdate> {
        self.inner.changed().await?;
        Ok(self.inner.borrow().clone())
    }

    pub fn latest_update(&mut self) -> PriceUpdate {
        self.inner.borrow().clone()
    }
}

#[derive(Clone, Debug, thiserror::Error)]
pub enum Error {
    #[error("BDX/BTC rate is not yet available")]
    NotYetAvailable,
    #[error("Permanently failed to retrieve BDX/BTC rate from CoinGecko")]
    PermanentFailure,
}

type PriceUpdate = Result<wire::PriceUpdate, Error>;

// reuse same wire shape as kraken so rate.rs needs no changes
pub mod wire {
    use bitcoin::Amount;

    #[derive(Clone, Debug)]
    pub struct PriceUpdate {
        pub ask: Amount,
    }
}
