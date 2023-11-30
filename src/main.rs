use log::info;
use protohackers_rs::{insecure_sockets, line_reversal, means_to_an_end, prime_time, smoke_test};
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
        }),
        tokio::spawn(async move {
            line_reversal::run("10007").await.unwrap();
        }),
        tokio::spawn(async move {
            insecure_sockets::run("10008").await.unwrap();
        })
    );

    Ok(())
}
