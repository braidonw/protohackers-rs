build:
  cargo build -r

run: build
  RUST_LOG=info ./target/release/protohackers
