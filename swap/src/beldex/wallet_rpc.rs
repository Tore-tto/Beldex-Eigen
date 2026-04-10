use ::beldex::Network;
use anyhow::{bail, Context, Error, Result};
use beldex_rpc::wallet::{BeldexWalletRpc as _, Client};
use big_bytes::BigByte;
use futures::{StreamExt, TryStreamExt};
use reqwest::header::CONTENT_LENGTH;
use reqwest::Url;
use serde::Deserialize;
use std::fmt::{Debug, Display, Formatter};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use std::{fmt, io};
use tokio::fs::{remove_file, OpenOptions};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio_util::codec::{BytesCodec, FramedRead};
use tokio_util::io::StreamReader;

// See: https://www.beldexworld.com/#nodes, https://beldex.fail
// We don't need any testnet nodes because we don't support testnet at all
const BELDEX_DAEMONS: [BeldexDaemon; 17] = [
    BeldexDaemon::new("bdx-node.cakewallet.com", 18081, Network::Mainnet),
    BeldexDaemon::new("nodex.monerujo.io", 18081, Network::Mainnet),
    BeldexDaemon::new("node.beldexworld.com", 18089, Network::Mainnet),
    BeldexDaemon::new("nodes.hashvault.pro", 18081, Network::Mainnet),
    BeldexDaemon::new("p2pmd.bdxvsbeast.com", 18081, Network::Mainnet),
    BeldexDaemon::new("node.beldexdevs.org", 18089, Network::Mainnet),
    BeldexDaemon::new("bdx-node-usa-east.cakewallet.com", 18081, Network::Mainnet),
    BeldexDaemon::new("bdx-node-uk.cakewallet.com", 18081, Network::Mainnet),
    BeldexDaemon::new("node.community.rino.io", 18081, Network::Mainnet),
    BeldexDaemon::new("testingjohnross.com", 20031, Network::Mainnet),
    BeldexDaemon::new("bdx.litepay.ch", 18081, Network::Mainnet),
    BeldexDaemon::new("node.trocador.app", 18089, Network::Mainnet),
    BeldexDaemon::new("stagenet.bdx-tw.org", 38081, Network::Stagenet),
    BeldexDaemon::new("node.beldexdevs.org", 38089, Network::Stagenet),
    BeldexDaemon::new("singapore.node.bdx.pm", 38081, Network::Stagenet),
    BeldexDaemon::new("bdx-lux.boldsuck.org", 38081, Network::Stagenet),
    BeldexDaemon::new("stagenet.community.rino.io", 38081, Network::Stagenet),
];

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
compile_error!("unsupported operating system");

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
const DOWNLOAD_URL: &str = "http://downloads.getmonero.org/cli/monero-mac-x64-v0.17.1.9.tar.bz2";

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
const DOWNLOAD_URL: &str = "http://downloads.getmonero.org/cli/monero-mac-x64-v0.17.1.9.tar.bz2";

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const DOWNLOAD_URL: &str = "https://github.com/Beldex-Coin/beldex/releases/download/v7.0.1/beldex-linux-x86_64-v7.0.1.tar.xz";

#[cfg(all(target_os = "linux", target_arch = "arm"))]
const DOWNLOAD_URL: &str =
    "https://github.com/Beldex-Coin/beldex/releases/download/v7.0.1/beldex-linux-x86_64-v7.0.1.tar.xz";

#[cfg(target_os = "windows")]
const DOWNLOAD_URL: &str = "https://downloads.getmonero.org/cli/monero-win-x64-v0.17.1.9.zip";

#[cfg(any(target_os = "macos", target_os = "linux"))]
const PACKED_FILE: &str = "beldex-wallet-rpc";

#[cfg(target_os = "windows")]
const PACKED_FILE: &str = "beldex-wallet-rpc.exe";

#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("beldex wallet rpc executable not found in downloaded archive")]
pub struct ExecutableNotFoundInArchive;

pub struct WalletRpcProcess {
    _child: Child,
    port: u16,
}

struct BeldexDaemon {
    address: &'static str,
    port: u16,
    network: Network,
}

impl BeldexDaemon {
    const fn new(address: &'static str, port: u16, network: Network) -> Self {
        Self {
            address,
            port,
            network,
        }
    }

    /// Checks if the Beldex daemon is available by sending a request to its `get_info` endpoint.
    async fn is_available(&self, client: &reqwest::Client) -> Result<bool, Error> {
        let url = format!("http://{}:{}/get_info", self.address, self.port);
        let res = client
            .get(url)
            .send()
            .await
            .context("Failed to send request to get_info endpoint")?;

        let json: BeldexDaemonGetInfoResponse = res
            .json()
            .await
            .context("Failed to deserialize daemon get_info response")?;

        let is_status_ok = json.status == "OK";
        let is_synchronized = json.synchronized;
        let is_correct_network = match self.network {
            Network::Mainnet => json.mainnet,
            Network::Stagenet => json.stagenet,
            Network::Testnet => json.testnet,
        };

        Ok(is_status_ok && is_synchronized && is_correct_network)
    }
}

impl Display for BeldexDaemon {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.address, self.port)
    }
}

#[derive(Deserialize)]
struct BeldexDaemonGetInfoResponse {
    status: String,
    synchronized: bool,
    mainnet: bool,
    stagenet: bool,
    testnet: bool,
}

/// Chooses an available Beldex daemon based on the specified network.
async fn choose_beldex_daemon(network: Network) -> Result<&'static BeldexDaemon, Error> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .https_only(false)
        .build()?;

    // We only want to check for daemons that match the specified network
    let network_matching_daemons = BELDEX_DAEMONS
        .iter()
        .filter(|daemon| daemon.network == network);

    for daemon in network_matching_daemons {
        match daemon.is_available(&client).await {
            Ok(true) => {
                tracing::debug!(%daemon, "Found available Beldex daemon");
                return Ok(daemon);
            }
            Err(err) => {
                tracing::debug!(%err, %daemon, "Failed to connect to Beldex daemon");
                continue;
            }
            Ok(false) => continue,
        }
    }

    bail!("No Beldex daemon could be found. Please specify one manually or try again later.")
}

impl WalletRpcProcess {
    pub fn endpoint(&self) -> Url {
        Url::parse(&format!("http://127.0.0.1:{}/json_rpc", self.port))
            .expect("Static url template is always valid")
    }

    pub fn kill(&mut self) -> io::Result<()> {
        self._child.start_kill()
    }
}

pub struct WalletRpc {
    working_dir: PathBuf,
}

impl WalletRpc {
    pub async fn new(working_dir: impl AsRef<Path>) -> Result<WalletRpc> {
        let working_dir = working_dir.as_ref();

        if !working_dir.exists() {
            tokio::fs::create_dir(working_dir).await?;
        }

        let beldex_wallet_rpc = WalletRpc {
            working_dir: working_dir.to_path_buf(),
        };

        if beldex_wallet_rpc.archive_path().exists() {
            remove_file(beldex_wallet_rpc.archive_path()).await?;
        }

        // if beldex-wallet-rpc doesn't exist then download it
        if !beldex_wallet_rpc.exec_path().exists() {
            let mut options = OpenOptions::new();
            let mut file = options
                .read(true)
                .write(true)
                .create_new(true)
                .open(beldex_wallet_rpc.archive_path())
                .await?;

            let response = reqwest::get(DOWNLOAD_URL).await?;

            let content_length = response.headers()[CONTENT_LENGTH]
                .to_str()
                .context("Failed to convert content-length to string")?
                .parse::<u64>()?;

            tracing::info!(
                "Downloading beldex-wallet-rpc ({})",
                content_length.big_byte(2)
            );

            let byte_stream = response
                .bytes_stream()
                .map_err(|err| std::io::Error::new(ErrorKind::Other, err));

            #[cfg(target_os = "linux")]
            let mut stream = FramedRead::new(
                async_compression::tokio::bufread::XzDecoder::new(StreamReader::new(byte_stream)),
                BytesCodec::new(),
            )
            .map_ok(|bytes| bytes.freeze());

            #[cfg(target_os = "macos")]
            let mut stream = FramedRead::new(
                async_compression::tokio::bufread::BzDecoder::new(StreamReader::new(byte_stream)),
                BytesCodec::new(),
            )
            .map_ok(|bytes| bytes.freeze());

            #[cfg(target_os = "windows")]
            let mut stream = FramedRead::new(StreamReader::new(byte_stream), BytesCodec::new())
                .map_ok(|bytes| bytes.freeze());

            while let Some(chunk) = stream.next().await {
                file.write(&chunk?).await?;
            }

            file.flush().await?;

            Self::extract_archive(&beldex_wallet_rpc).await?;
        }
        Ok(beldex_wallet_rpc)
    }

    pub async fn run(
        &self,
        network: Network,
        daemon_address: Option<String>,
    ) -> Result<WalletRpcProcess> {
        let port = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await?
            .local_addr()?
            .port();

        let daemon_address = match daemon_address {
            Some(daemon_address) => daemon_address,
            None => choose_beldex_daemon(network).await?.to_string(),
        };

        tracing::debug!(
            %daemon_address,
            %port,
            "Starting beldex-wallet-rpc"
        );

        let network_flag = match network {
            Network::Mainnet => {
                vec![]
            }
            Network::Stagenet => {
                vec!["--testnet"]
            }
            Network::Testnet => {
                vec!["--testnet"]
            }
        };

        let mut child = Command::new(self.exec_path())
            .env("LANG", "en_AU.UTF-8")
            .stdout(Stdio::piped())
            .kill_on_drop(true)
            .args(network_flag)
            .arg("--daemon-address")
            .arg(daemon_address)
            .arg("--rpc-bind-port")
            .arg(format!("{}", port))
            .arg("--disable-rpc-login")
            .arg("--wallet-dir")
            .arg(self.working_dir.join("beldex-data"))
            .spawn()?;

        let stdout = child
            .stdout
            .take()
            .expect("beldex wallet rpc stdout was not piped parent process");

        let mut reader = BufReader::new(stdout).lines();

        #[cfg(not(target_os = "windows"))]
        while let Some(line) = reader.next_line().await? {
            if line.contains("Starting wallet RPC server") {
                break;
            }
        }

        // If we do not hear from the beldex_wallet_rpc process for 3 seconds we assume
        // it is ready
        #[cfg(target_os = "windows")]
        while let Ok(line) =
            tokio::time::timeout(std::time::Duration::from_secs(3), reader.next_line()).await
        {
            line?;
        }

        // Send a json rpc request to make sure beldex_wallet_rpc is ready
        Client::localhost(port)?.get_version().await?;

        Ok(WalletRpcProcess {
            _child: child,
            port,
        })
    }

    fn archive_path(&self) -> PathBuf {
        self.working_dir.join("beldex-cli-wallet.archive")
    }

    fn exec_path(&self) -> PathBuf {
        self.working_dir.join(PACKED_FILE)
    }

    #[cfg(not(target_os = "windows"))]
    async fn extract_archive(beldex_wallet_rpc: &Self) -> Result<()> {
        use tokio_tar::Archive;

        let mut options = OpenOptions::new();
        let file = options
            .read(true)
            .open(beldex_wallet_rpc.archive_path())
            .await?;

        let mut ar = Archive::new(file);
        let mut entries = ar.entries()?;

        loop {
            match entries.next().await {
                Some(file) => {
                    let mut f = file?;
                    if f.path()?
                        .to_str()
                        .context("Could not find convert path to str in tar ball")?
                        .contains(PACKED_FILE)
                    {
                        f.unpack(beldex_wallet_rpc.exec_path()).await?;
                        break;
                    }
                }
                None => bail!(ExecutableNotFoundInArchive),
            }
        }

        remove_file(beldex_wallet_rpc.archive_path()).await?;

        Ok(())
    }

    #[cfg(target_os = "windows")]
    async fn extract_archive(beldex_wallet_rpc: &Self) -> Result<()> {
        use std::fs::File;
        use tokio::task::JoinHandle;
        use zip::ZipArchive;

        let archive_path = beldex_wallet_rpc.archive_path();
        let exec_path = beldex_wallet_rpc.exec_path();

        let extract: JoinHandle<Result<()>> = tokio::task::spawn_blocking(|| {
            let file = File::open(archive_path)?;
            let mut zip = ZipArchive::new(file)?;

            let name = zip
                .file_names()
                .find(|name| name.contains(PACKED_FILE))
                .context(ExecutableNotFoundInArchive)?
                .to_string();

            let mut rpc = zip.by_name(&name)?;
            let mut file = File::create(exec_path)?;
            std::io::copy(&mut rpc, &mut file)?;
            Ok(())
        });
        extract.await??;

        remove_file(beldex_wallet_rpc.archive_path()).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extract_host_and_port(address: String) -> (&'static str, u16) {
        let parts: Vec<&str> = address.split(':').collect();

        if parts.len() == 2 {
            let host = parts[0].to_string();
            let port = parts[1].parse::<u16>().unwrap();
            let static_str_host: &'static str = Box::leak(host.into_boxed_str());
            return (static_str_host, port);
        }
        panic!("Could not extract host and port from address: {}", address)
    }

    #[tokio::test]
    async fn test_is_daemon_available_success() {
        let mut server = mockito::Server::new_async().await;

        let _ = server
            .mock("GET", "/get_info")
            .with_status(200)
            .with_body(
                r#"
                {
                    "status": "OK",
                    "synchronized": true,
                    "mainnet": true,
                    "stagenet": false,
                    "testnet": false
                }
                "#,
            )
            .create();

        let (host, port) = extract_host_and_port(server.host_with_port());

        let client = reqwest::Client::new();
        let result = BeldexDaemon::new(host, port, Network::Mainnet)
            .is_available(&client)
            .await;

        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn test_is_daemon_available_wrong_network_failure() {
        let mut server = mockito::Server::new_async().await;

        let _ = server
            .mock("GET", "/get_info")
            .with_status(200)
            .with_body(
                r#"
                {
                    "status": "OK",
                    "synchronized": true,
                    "mainnet": true,
                    "stagenet": false,
                    "testnet": false
                }
                "#,
            )
            .create();

        let (host, port) = extract_host_and_port(server.host_with_port());

        let client = reqwest::Client::new();
        let result = BeldexDaemon::new(host, port, Network::Stagenet)
            .is_available(&client)
            .await;

        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[tokio::test]
    async fn test_is_daemon_available_not_synced_failure() {
        let mut server = mockito::Server::new_async().await;

        let _ = server
            .mock("GET", "/get_info")
            .with_status(200)
            .with_body(
                r#"
                {
                    "status": "OK",
                    "synchronized": false,
                    "mainnet": true,
                    "stagenet": false,
                    "testnet": false
                }
                "#,
            )
            .create();

        let (host, port) = extract_host_and_port(server.host_with_port());

        let client = reqwest::Client::new();
        let result = BeldexDaemon::new(host, port, Network::Mainnet)
            .is_available(&client)
            .await;

        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[tokio::test]
    async fn test_is_daemon_available_network_error_failure() {
        let client = reqwest::Client::new();
        let result = BeldexDaemon::new("does.not.exist.com", 18081, Network::Mainnet)
            .is_available(&client)
            .await;

        assert!(result.is_err());
    }
}
