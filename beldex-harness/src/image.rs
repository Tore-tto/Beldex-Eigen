use testcontainers::{core::WaitFor, Image, ImageArgs};

pub const BELDEXD_DAEMON_CONTAINER_NAME: &str = "beldexd";
pub const BELDEXD_DEFAULT_NETWORK: &str = "beldex_network";

/// The port we use for all RPC communication.
///
/// This is the default when running beldexd.
/// For `beldex-wallet-rpc` we always need to specify a port. To make things
/// simpler, we just specify the same one. They are in different containers so
/// this doesn't matter.
pub const RPC_PORT: u16 = 18081;

#[derive(Clone, Copy, Debug, Default)]
pub struct Beldexd;

impl Image for Beldexd {
    type Args = BeldexdArgs;

    fn name(&self) -> String {
        "bafdb1a40140".into()
    }

    fn tag(&self) -> String {
        "latest".into()
    }

    fn ready_conditions(&self) -> Vec<WaitFor> {
        vec![WaitFor::message_on_stdout("RPC server started ok")]
    }

    fn entrypoint(&self) -> Option<String> {
        Some("".to_owned()) // an empty entrypoint disables the entrypoint
                            // script and gives us full control
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BeldexWalletRpc;

impl Image for BeldexWalletRpc {
    type Args = BeldexWalletRpcArgs;

    fn name(&self) -> String {
        "bafdb1a40140".into()
    }

    fn tag(&self) -> String {
        "latest".into()
    }

    fn ready_conditions(&self) -> Vec<WaitFor> {
        vec![WaitFor::message_on_stdout("Run server thread name: RPC")]
    }

    fn entrypoint(&self) -> Option<String> {
        Some("".to_owned()) // an empty entrypoint disables the entrypoint
                            // script and gives us full control
    }
}

impl BeldexWalletRpc {
    pub fn new(name: &str, daemon_address: String) -> (Self, BeldexWalletRpcArgs) {
        let args = BeldexWalletRpcArgs::new(name, daemon_address);
        (Self, args)
    }
}

#[derive(Debug, Clone)]
pub struct BeldexdArgs {
    pub regtest: bool,
    pub offline: bool,
    pub rpc_payment_allow_free_loopback: bool,
    pub confirm_external_bind: bool,
    pub no_igd: bool,
    pub hide_my_port: bool,
    pub rpc_bind_ip: String,
    pub data_dir: String,
}

impl Default for BeldexdArgs {
    fn default() -> Self {
        Self {
            regtest: true,
            offline: true,
            rpc_payment_allow_free_loopback: true,
            confirm_external_bind: true,
            no_igd: true,
            hide_my_port: true,
            rpc_bind_ip: "0.0.0.0".to_string(),
            data_dir: "/beldex".to_string(),
        }
    }
}

impl IntoIterator for BeldexdArgs {
    type Item = String;
    type IntoIter = ::std::vec::IntoIter<String>;

    fn into_iter(self) -> <Self as IntoIterator>::IntoIter {
        let mut args = vec![
            "beldexd".to_string(),
            "--log-level=4".to_string(),
            "--non-interactive".to_string(),
        ];

        if self.regtest {
            args.push("--regtest".to_string())
        }

        if self.offline {
            args.push("--offline".to_string())
        }

        if self.rpc_payment_allow_free_loopback {
            args.push("--rpc-payment-allow-free-loopback".to_string())
        }

        if self.confirm_external_bind {
            args.push("--confirm-external-bind".to_string())
        }

        if self.no_igd {
            args.push("--no-igd".to_string())
        }

        if self.hide_my_port {
            args.push("--hide-my-port".to_string())
        }

        if !self.rpc_bind_ip.is_empty() {
            args.push(format!("--rpc-bind-ip={}", self.rpc_bind_ip));
        }

        if !self.data_dir.is_empty() {
            args.push(format!("--data-dir={}", self.data_dir));
        }

        args.into_iter()
    }
}

impl ImageArgs for BeldexdArgs {
    fn into_iterator(self) -> Box<dyn Iterator<Item = String>> {
        Box::new(self.into_iter())
    }
}

#[derive(Debug, Clone)]
pub struct BeldexWalletRpcArgs {
    pub disable_rpc_login: bool,
    pub confirm_external_bind: bool,
    pub wallet_dir: String,
    pub rpc_bind_ip: String,
    pub daemon_address: String,
}

impl BeldexWalletRpcArgs {
    pub fn new(wallet_name: &str, daemon_address: String) -> Self {
        Self {
            disable_rpc_login: true,
            confirm_external_bind: true,
            wallet_dir: wallet_name.into(),
            rpc_bind_ip: "0.0.0.0".into(),
            daemon_address,
        }
    }
}

impl IntoIterator for BeldexWalletRpcArgs {
    type Item = String;
    type IntoIter = ::std::vec::IntoIter<String>;

    fn into_iter(self) -> <Self as IntoIterator>::IntoIter {
        let mut args = vec![
            "beldex-wallet-rpc".to_string(),
            format!("--wallet-dir={}", self.wallet_dir),
            format!("--daemon-address={}", self.daemon_address),
            format!("--rpc-bind-port={}", RPC_PORT),
            "--log-level=4".to_string(),
            "--allow-mismatched-daemon-version".to_string(), /* https://github.com/beldex-project/beldex/issues/8600 */
        ];

        if self.disable_rpc_login {
            args.push("--disable-rpc-login".to_string())
        }

        if self.confirm_external_bind {
            args.push("--confirm-external-bind".to_string())
        }

        if !self.rpc_bind_ip.is_empty() {
            args.push(format!("--rpc-bind-ip={}", self.rpc_bind_ip));
        }

        args.into_iter()
    }
}

impl ImageArgs for BeldexWalletRpcArgs {
    fn into_iterator(self) -> Box<dyn Iterator<Item = String>> {
        Box::new(self.into_iter())
    }
}
