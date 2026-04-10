use beldex_harness::Beldex;
use beldex_rpc::beldexd::BeldexdRpc as _;
use std::time::Duration;
use testcontainers::clients::Cli;
use tokio::time;
use tracing_subscriber::util::SubscriberInitExt;

#[tokio::test]
async fn init_miner_and_mine_to_miner_address() {
    let _guard = tracing_subscriber::fmt()
        .with_env_filter("warn,test=debug,beldex_harness=debug,beldex_rpc=debug")
        .set_default();

    let tc = Cli::default();
    let (beldex, _beldexd_container, _wallet_containers) = Beldex::new(&tc, vec![]).await.unwrap();

    // beldex.init_and_start_miner().await.unwrap();

    let beldexd = beldex.beldexd();
    let miner_wallet = beldex.wallet("miner").unwrap();

    let got_miner_balance = miner_wallet.balance().await.unwrap();
    assert!(got_miner_balance > 0);

    time::sleep(Duration::from_millis(1010)).await;

    // after a bit more than 1 sec another block should have been mined
    let block_height = beldexd.client().get_block_count().await.unwrap().count;

    assert!(block_height > 70);
}
