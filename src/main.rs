use protohackers_rs::smoke_test;

#[tokio::main]
async fn main() {
    println!("Running Protohackers Servers");
    smoke_test::run("10000").await.unwrap();
}
