use log::info;
use protohackers_rs::{means_to_an_end, prime_time, smoke_test};
use tokio::join;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    info!("Running Protohackers Servers");

    let _ = join!(
        tokio::spawn(async move {
            smoke_test::run("10000").await.unwrap();
        }),
        tokio::spawn(async move { prime_time::run("10001").await.unwrap() }),
        tokio::spawn(async move {
            means_to_an_end::run("10002").await.unwrap();
        })
    );

    Ok(())
}
