#![warn(
    unused_extern_crates,
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
#![allow(non_snake_case)]

use anyhow::{bail, Context, Result};
use comfy_table::Table;
use libp2p::core::multiaddr::Protocol;
use libp2p::core::Multiaddr;
use libp2p::swarm::AddressScore;
use libp2p::Swarm;
use std::convert::TryInto;
use std::env;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use structopt::clap;
use structopt::clap::ErrorKind;
use swap::asb::command::{parse_args, Arguments, Command};
use swap::asb::config::{
    initial_setup, query_user_for_initial_config, read_config, Config, ConfigNotInitialized,
    PriceSource,
};
use swap::asb::{
    cancel, punish, redeem, refund, safely_abort, CoinGeckoRate, DynamicRate, EventLoop, Finality,
    KrakenRate,
};
use swap::coingecko;
use swap::common::tracing_util::Format;
use swap::common::{self, check_latest_version, get_logs};
use swap::database::open_db;
use swap::network::rendezvous::BeldexBtcNamespace;
use swap::network::swarm;
use swap::protocol::alice::{run, AliceState};
use swap::seed::Seed;
use swap::tor::AuthenticatedClient;
use swap::{bitcoin, kraken, beldex, tor};
use tracing_subscriber::filter::LevelFilter;

const DEFAULT_WALLET_NAME: &str = "asb-wallet";

#[tokio::main]
pub async fn main() -> Result<()> {
    let Arguments {
        testnet,
        json,
        config_path,
        env_config,
        cmd,
    } = match parse_args(env::args_os()) {
        Ok(args) => args,
        Err(e) => {
            // make sure to display the clap error message it exists
            if let Some(clap_err) = e.downcast_ref::<clap::Error>() {
                if let ErrorKind::HelpDisplayed | ErrorKind::VersionDisplayed = clap_err.kind {
                    println!("{}", clap_err.message);
                    std::process::exit(0);
                }
            }
            bail!(e);
        }
    };

    // warn if we're not on the latest version
    if let Err(e) = check_latest_version(env!("CARGO_PKG_VERSION")).await {
        eprintln!("{}", e);
    }

    // read config from the specified path
    let config = match read_config(config_path.clone())? {
        Ok(config) => config,
        Err(ConfigNotInitialized {}) => {
            initial_setup(config_path.clone(), query_user_for_initial_config(testnet)?)?;
            read_config(config_path)?.expect("after initial setup config can be read")
        }
    };

    // initialize tracing
    let format = if json { Format::Json } else { Format::Raw };
    let log_dir = config.data.dir.join("logs");
    common::tracing_util::init(LevelFilter::DEBUG, format, log_dir).expect("initialize tracing");

    // check for conflicting env / config values
    if config.beldex.network != env_config.beldex_network {
        bail!(format!(
            "Expected beldex network in config file to be {:?} but was {:?}",
            env_config.beldex_network, config.beldex.network
        ));
    }
    if config.bitcoin.network != env_config.bitcoin_network {
        bail!(format!(
            "Expected bitcoin network in config file to be {:?} but was {:?}",
            env_config.bitcoin_network, config.bitcoin.network
        ));
    }

    let db = open_db(config.data.dir.join("sqlite")).await?;

    let seed =
        Seed::from_file_or_generate(&config.data.dir).expect("Could not retrieve/initialize seed");

    match cmd {
        Command::Start { resume_only } => {
            // check and warn for duplicate rendezvous points
            let mut rendezvous_addrs = config.network.rendezvous_point.clone();
            let prev_len = rendezvous_addrs.len();
            rendezvous_addrs.sort();
            rendezvous_addrs.dedup();
            let new_len = rendezvous_addrs.len();

            if new_len < prev_len {
                tracing::warn!(
                    "`rendezvous_point` config has {} duplicate entries, they are being ignored.",
                    prev_len - new_len
                );
            }

            // initialize beldex wallet
            let beldex_wallet = init_beldex_wallet(&config, env_config).await?;
            let beldex_address = beldex_wallet.get_main_address();
            tracing::info!(%beldex_address, "Beldex wallet address");

            // check beldex balance
            let beldex = beldex_wallet.get_balance().await?;
            match (beldex.balance, beldex.unlocked_balance) {
                (0, _) => {
                    tracing::warn!(
                        %beldex_address,
                        "The Beldex balance is 0, make sure to deposit funds at",
                    )
                }
                (total, 0) => {
                    let total = beldex::Amount::from_atomic(total);
                    tracing::warn!(
                        %total,
                        "Unlocked Beldex balance is 0, total balance is",
                    )
                }
                (total, unlocked) => {
                    let total = beldex::Amount::from_atomic(total);
                    let unlocked = beldex::Amount::from_atomic(unlocked);
                    tracing::info!(%total, %unlocked, "Beldex wallet balance");
                }
            }

            // init bitcoin wallet
            let bitcoin_wallet = init_bitcoin_wallet(&config, &seed, env_config).await?;
            let bitcoin_balance = bitcoin_wallet.balance().await?;
            tracing::info!(%bitcoin_balance, "Bitcoin wallet balance");

            let price_rate = match config.maker.price_ticker_source {
                PriceSource::Kraken => {
                    let kraken_price_updates = kraken::connect(config.maker.price_ticker_ws_url.clone())?;
                    DynamicRate::Kraken(KrakenRate::new(config.maker.ask_spread, kraken_price_updates))
                }
                PriceSource::Coingecko => {
                    let coingecko_price_updates = coingecko::connect(Duration::from_secs(60))?; // Poll every minute
                    DynamicRate::CoinGecko(CoinGeckoRate::new(config.maker.ask_spread, coingecko_price_updates))
                }
            };

            // setup Tor hidden services
            let tor_client =
                tor::Client::new(config.tor.socks5_port).with_control_port(config.tor.control_port);
            let _ac = match tor_client.assert_tor_running().await {
                Ok(_) => {
                    tracing::info!("Setting up Tor hidden service");
                    let ac =
                        register_tor_services(config.network.clone().listen, tor_client, &seed)
                            .await?;
                    Some(ac)
                }
                Err(_) => {
                    tracing::warn!("Tor not found. Running on clear net");
                    None
                }
            };
            let namespace = BeldexBtcNamespace::from_is_testnet(testnet);

            let mut swarm = swarm::asb(
                &seed,
                config.maker.min_buy_btc,
                config.maker.max_buy_btc,
                price_rate.clone(),
                resume_only,
                env_config,
                namespace,
                &rendezvous_addrs,
            )?;

            for listen in config.network.listen.clone() {
                Swarm::listen_on(&mut swarm, listen.clone())
                    .with_context(|| format!("Failed to listen on network interface {}", listen))?;
            }

            tracing::info!(peer_id = %swarm.local_peer_id(), "Network layer initialized");

            for external_address in config.network.external_addresses {
                let _ = Swarm::add_external_address(
                    &mut swarm,
                    external_address,
                    AddressScore::Infinite,
                );
            }

            let (event_loop, mut swap_receiver): (EventLoop<DynamicRate>, _) = EventLoop::new(
                swarm,
                env_config,
                Arc::new(bitcoin_wallet),
                Arc::new(beldex_wallet),
                db,
                price_rate.clone(),
                config.maker.min_buy_btc,
                config.maker.max_buy_btc,
                config.maker.external_bitcoin_redeem_address,
            )
            .unwrap();

            tokio::spawn(async move {
                while let Some(swap) = swap_receiver.recv().await {
                    let rate = price_rate.clone();
                    tokio::spawn(async move {
                        let swap_id = swap.swap_id;
                        match run(swap, rate).await {
                            Ok(state) => {
                                tracing::debug!(%swap_id, final_state=%state, "Swap completed")
                            }
                            Err(error) => {
                                tracing::error!(%swap_id, "Swap failed: {:#}", error)
                            }
                        }
                    });
                }
            });

            event_loop.run().await;
        }
        Command::History => {
            let mut table = Table::new();

            table.set_header(vec!["SWAP ID", "STATE"]);

            for (swap_id, state) in db.all().await? {
                let state: AliceState = state.try_into()?;
                table.add_row(vec![swap_id.to_string(), state.to_string()]);
            }

            println!("{}", table);
        }
        Command::Config => {
            let config_json = serde_json::to_string_pretty(&config)?;
            println!("{}", config_json);
        }
        Command::Logs {
            logs_dir,
            swap_id,
            redact,
        } => {
            let dir = logs_dir.unwrap_or(config.data.dir.join("logs"));

            let log_messages = get_logs(dir, swap_id, redact).await?;

            for msg in log_messages {
                println!("{msg}");
            }
        }
        Command::WithdrawBtc { amount, address } => {
            let bitcoin_wallet = init_bitcoin_wallet(&config, &seed, env_config).await?;

            let amount = match amount {
                Some(amount) => amount,
                None => {
                    bitcoin_wallet
                        .max_giveable(address.script_pubkey().len())
                        .await?
                }
            };

            let psbt = bitcoin_wallet
                .send_to_address(address, amount, None)
                .await?;
            let signed_tx = bitcoin_wallet.sign_and_finalize(psbt).await?;

            bitcoin_wallet.broadcast(signed_tx, "withdraw").await?;
        }
        Command::Balance => {
            let beldex_wallet = init_beldex_wallet(&config, env_config).await?;
            let beldex_balance = beldex_wallet.get_balance().await?;
            tracing::info!(%beldex_balance);

            let bitcoin_wallet = init_bitcoin_wallet(&config, &seed, env_config).await?;
            let bitcoin_balance = bitcoin_wallet.balance().await?;
            tracing::info!(%bitcoin_balance);
            tracing::info!(%bitcoin_balance, %beldex_balance, "Current balance");
        }
        Command::Cancel { swap_id } => {
            let bitcoin_wallet = init_bitcoin_wallet(&config, &seed, env_config).await?;

            let (txid, _) = cancel(swap_id, Arc::new(bitcoin_wallet), db).await?;

            tracing::info!("Cancel transaction successfully published with id {}", txid);
        }
        Command::Refund { swap_id } => {
            let bitcoin_wallet = init_bitcoin_wallet(&config, &seed, env_config).await?;
            let beldex_wallet = init_beldex_wallet(&config, env_config).await?;

            refund(
                swap_id,
                Arc::new(bitcoin_wallet),
                Arc::new(beldex_wallet),
                db,
            )
            .await?;

            tracing::info!("Beldex successfully refunded");
        }
        Command::Punish { swap_id } => {
            let bitcoin_wallet = init_bitcoin_wallet(&config, &seed, env_config).await?;

            let (txid, _) = punish(swap_id, Arc::new(bitcoin_wallet), db).await?;

            tracing::info!("Punish transaction successfully published with id {}", txid);
        }
        Command::SafelyAbort { swap_id } => {
            safely_abort(swap_id, db).await?;

            tracing::info!("Swap safely aborted");
        }
        Command::Redeem {
            swap_id,
            do_not_await_finality,
        } => {
            let bitcoin_wallet = init_bitcoin_wallet(&config, &seed, env_config).await?;

            let (txid, _) = redeem(
                swap_id,
                Arc::new(bitcoin_wallet),
                db,
                Finality::from_bool(do_not_await_finality),
            )
            .await?;

            tracing::info!("Redeem transaction successfully published with id {}", txid);
        }
        Command::ExportBitcoinWallet => {
            let bitcoin_wallet = init_bitcoin_wallet(&config, &seed, env_config).await?;
            let wallet_export = bitcoin_wallet.wallet_export("asb").await?;
            println!("{}", wallet_export.to_string())
        }
    }

    Ok(())
}

async fn init_bitcoin_wallet(
    config: &Config,
    seed: &Seed,
    env_config: swap::env::Config,
) -> Result<bitcoin::Wallet> {
    tracing::debug!("Opening Bitcoin wallet");
    let data_dir = &config.data.dir;
    let wallet = bitcoin::Wallet::new(
        config.bitcoin.electrum_rpc_url.clone(),
        data_dir,
        seed.derive_extended_private_key(env_config.bitcoin_network)?,
        env_config,
        config.bitcoin.target_block,
    )
    .await
    .context("Failed to initialize Bitcoin wallet")?;

    wallet.sync().await?;

    Ok(wallet)
}

async fn init_beldex_wallet(
    config: &Config,
    env_config: swap::env::Config,
) -> Result<beldex::Wallet> {
    tracing::debug!("Opening Beldex wallet");
    let wallet = beldex::Wallet::open_or_create(
        config.beldex.wallet_rpc_url.clone(),
        DEFAULT_WALLET_NAME.to_string(),
        env_config,
    )
    .await?;

    Ok(wallet)
}

/// Registers a hidden service for each network.
/// Note: Once ac goes out of scope, the services will be de-registered.
async fn register_tor_services(
    networks: Vec<Multiaddr>,
    tor_client: tor::Client,
    seed: &Seed,
) -> Result<AuthenticatedClient> {
    let mut ac = tor_client.into_authenticated_client().await?;

    let hidden_services_details = networks
        .iter()
        .flat_map(|network| {
            network.iter().map(|protocol| match protocol {
                Protocol::Tcp(port) => Some((
                    port,
                    SocketAddr::new(IpAddr::from(Ipv4Addr::new(127, 0, 0, 1)), port),
                )),
                _ => {
                    // We only care for Tcp for now.
                    None
                }
            })
        })
        .flatten()
        .collect::<Vec<_>>();

    let key = seed.derive_torv3_key();

    ac.add_services(&hidden_services_details, &key).await?;

    let onion_address = key
        .public()
        .get_onion_address()
        .get_address_without_dot_onion();

    hidden_services_details.iter().for_each(|(port, _)| {
        let onion_address = format!("/onion3/{}:{}", onion_address, port);
        tracing::info!(%onion_address, "Successfully created hidden service");
    });

    Ok(ac)
}
