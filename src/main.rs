use protohackers_rs::{prime_time, smoke_test};
use tokio::join;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Running Protohackers Servers");

    let _ = join!(
        tokio::spawn(async move { prime_time::run("10001").await.unwrap() }),
        tokio::spawn(async move {
            smoke_test::run("10000").await.unwrap();
        })
    );

    Ok(())
}
