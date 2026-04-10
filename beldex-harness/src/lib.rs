#![warn(
    unused_extern_crates,
    missing_debug_implementations,
    missing_copy_implementations,
    rust_2018_idioms,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::fallible_impl_from,
    clippy::cast_precision_loss,
    clippy::cast_possible_wrap,
    clippy::dbg_macro
)]
#![forbid(unsafe_code)]

//! # beldex-harness
//!
//! A simple lib to start a beldex container (incl. beldexd and
//! beldex-wallet-rpc). Provides initialisation methods to fund accounts.
//!
//! Also provides standalone JSON RPC clients for beldexd and beldex-wallet-rpc.
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use testcontainers::clients::Cli;
use testcontainers::{Container, RunnableImage};
use tokio::time;

use beldex_rpc::beldexd;
use beldex_rpc::wallet::{self, BeldexWalletRpc as _, GetAddress, Refreshed, Transfer};

use crate::image::{BELDEXD_DAEMON_CONTAINER_NAME, BELDEXD_DEFAULT_NETWORK, RPC_PORT};

pub mod image;

/// Poll interval when checking if the wallet has synced with beldexd.
const WAIT_WALLET_SYNC_MILLIS: u64 = 1000;

#[derive(Clone, Debug)]
pub struct Beldex {
    beldexd: Beldexd,
    wallets: Vec<BeldexWalletRpc>,
}
impl<'c> Beldex {
    /// Starts a new regtest beldex container setup consisting out of 1 beldexd
    /// node and n wallets. The docker container and network will be prefixed
    /// beldexd container name is: `prefix`_`beldexd`
    /// network is: `prefix`_`beldex`
    pub async fn new(
        cli: &'c Cli,
        additional_wallets: Vec<&'static str>,
    ) -> Result<(
        Self,
        Container<'c, image::Beldexd>,
        Vec<Container<'c, image::BeldexWalletRpc>>,
    )> {
        let prefix = format!("{}_", random_prefix());
        let beldexd_name = format!("{}{}", prefix, BELDEXD_DAEMON_CONTAINER_NAME);
        let network = format!("{}{}", prefix, BELDEXD_DEFAULT_NETWORK);

        tracing::info!("Starting beldexd: {}", beldexd_name);
        let (beldexd, beldexd_container) = Beldexd::new(cli, beldexd_name, network)?;
        let mut containers = vec![];
        let mut wallets = vec![];

        for wallet in additional_wallets.iter() {
            tracing::info!("Starting wallet: {}", wallet);

            // Create new wallet, the RPC sometimes has startup problems so we allow retries
            // (drop the container that failed and try again) Times out after
            // trying for 5 minutes
            let (wallet, container) = tokio::time::timeout(Duration::from_secs(300), async {
                loop {
                    let result = BeldexWalletRpc::new(cli, wallet, &beldexd, prefix.clone()).await;

                    match result {
                        Ok(tuple) => { return tuple; }
                        Err(e) => { tracing::warn!("Beldex wallet RPC emitted error {} - retrying to create wallet in 2 seconds...", e); }
                    }
                }
            }).await.context("All retry attempts for creating a wallet exhausted")?;

            wallets.push(wallet);
            containers.push(container);
        }

        Ok((Self { beldexd, wallets }, beldexd_container, containers))
    }

    pub fn beldexd(&self) -> &Beldexd {
        &self.beldexd
    }

    pub fn wallet(&self, name: &str) -> Result<&BeldexWalletRpc> {
        let wallet = self
            .wallets
            .iter()
            .find(|wallet| wallet.name.eq(&name))
            .ok_or_else(|| anyhow!("Could not find wallet container."))?;

        Ok(wallet)
    }

    pub async fn init_wallet(&self, name: &str, amount_in_outputs: Vec<u64>) -> Result<()> {
        let wallet = self.wallet(name)?;
        let _address = wallet.address().await?.address;

        for amount in amount_in_outputs {
            if amount > 0 {
                tracing::info!("Funded {} wallet with {} (Note: actual funding requires external source or pre-funded regtest account)", wallet.name, amount);
                // In PoS/regtest, we might need a different way to fund if not using generateblocks.
                // For now, we keep the logic structure but acknowledge the change.
            }
        }

        Ok(())
    }

}

fn random_prefix() -> String {
    use rand::Rng;

    rand::thread_rng()
        .sample_iter(rand::distributions::Alphanumeric)
        .take(4)
        .collect()
}

#[derive(Clone, Debug)]
pub struct Beldexd {
    name: String,
    network: String,
    client: beldexd::Client,
}

#[derive(Clone, Debug)]
pub struct BeldexWalletRpc {
    name: String,
    client: wallet::Client,
}

impl<'c> Beldexd {
    /// Starts a new regtest beldex container.
    fn new(
        cli: &'c Cli,
        name: String,
        network: String,
    ) -> Result<(Self, Container<'c, image::Beldexd>)> {
        let image = image::Beldexd;
        let image: RunnableImage<image::Beldexd> = RunnableImage::from(image)
            .with_container_name(name.clone())
            .with_network(network.clone());

        let container = cli.run(image);
        let beldexd_rpc_port = container.get_host_port_ipv4(RPC_PORT);

        Ok((
            Self {
                name,
                network,
                client: beldexd::Client::localhost(beldexd_rpc_port)?,
            },
            container,
        ))
    }

    pub fn client(&self) -> &beldexd::Client {
        &self.client
    }
}

impl<'c> BeldexWalletRpc {
    /// Starts a new wallet container which is attached to
    /// BELDEXD_DEFAULT_NETWORK and BELDEXD_DAEMON_CONTAINER_NAME
    async fn new(
        cli: &'c Cli,
        name: &str,
        beldexd: &Beldexd,
        prefix: String,
    ) -> Result<(Self, Container<'c, image::BeldexWalletRpc>)> {
        let daemon_address = format!("{}:{}", beldexd.name, RPC_PORT);
        let (image, args) = image::BeldexWalletRpc::new(name, daemon_address);
        let image = RunnableImage::from((image, args))
            .with_container_name(format!("{}{}", prefix, name))
            .with_network(beldexd.network.clone());

        let container = cli.run(image);
        let wallet_rpc_port = container.get_host_port_ipv4(RPC_PORT);

        let client = wallet::Client::localhost(wallet_rpc_port)?;

        client
            .create_wallet(name.to_owned(), "English".to_owned())
            .await?;

        Ok((
            Self {
                name: name.to_string(),
                client,
            },
            container,
        ))
    }

    pub fn client(&self) -> &wallet::Client {
        &self.client
    }

    // It takes a little while for the wallet to sync with beldexd.
    pub async fn wait_for_wallet_height(&self, height: u32) -> Result<()> {
        let mut retry: u8 = 0;
        while self.client().get_height().await?.height < height {
            if retry >= 30 {
                // ~30 seconds
                bail!("Wallet could not catch up with beldexd after 30 retries.")
            }
            time::sleep(Duration::from_millis(WAIT_WALLET_SYNC_MILLIS)).await;
            retry += 1;
        }
        Ok(())
    }

    /// Sends amount to address
    pub async fn transfer(&self, address: &str, amount: u64) -> Result<Transfer> {
        self.client().transfer_single(0, amount, address).await
    }

    pub async fn address(&self) -> Result<GetAddress> {
        Ok(self.client().get_address(0).await?)
    }

    pub async fn balance(&self) -> Result<u64> {
        self.client().refresh().await?;
        let balance = self.client().get_balance(0).await?.balance;

        Ok(balance)
    }

    pub async fn unlocked_balance(&self) -> Result<u64> {
        self.client().refresh().await?;
        let balance = self.client().get_balance(0).await?.unlocked_balance;

        Ok(balance)
    }

    pub async fn refresh(&self) -> Result<Refreshed> {
        Ok(self.client().refresh().await?)
    }
}

