from rust as builder
env RUSTUP_HOME=/root/.rustup
env CARGO_HOME=/root/.cargo

run rustup default stable
# run rustup component add rust-analyzer
copy Cargo.toml Cargo.lock .
run mkdir src
run echo "fn main() {}" > src/lib.rs
run cargo build --release
copy ./src ./src
run touch src/lib.rs
run cargo install --locked --target-dir target --path . --root /install

from ubuntu:24.04
run apt-get update && apt-get install -y ca-certificates && apt-get clean
arg install_prefix=/usr/local
workdir $install_prefix
copy --from=builder /install ./
