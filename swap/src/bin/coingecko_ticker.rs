use anyhow::{Context, Result};
use url::Url;
use swap::coingecko;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    let mut ticker = coingecko::connect(Duration::from_secs(60))?;    

    loop {
        match ticker.wait_for_next_update().await? {
            Ok(update) => println!("Price update: {}", update.ask),
            Err(e) =>  println!("Error: {}", e),
        }
    }
}
