default:
  @just --list 

fmt:
    @cargo +nightly fmt --all

lint:
    @cargo clippy --workspace --all-targets --all-features -- -D warnings

check:
    @cargo check --workspace --all-targets --all-features

test:
    @cargo test --workspace --all-features -- --test-threads=1   

