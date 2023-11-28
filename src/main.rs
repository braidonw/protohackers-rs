use protohackers_rs::{prime_time, smoke_test};

#[tokio::main]
async fn main() {
    println!("Running Protohackers Servers");
    smoke_test::run("10000").await.unwrap();
    prime_time::run("10001").await.unwrap();
}
