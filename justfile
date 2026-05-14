default:
    @just --list

build:
    cargo build --release

install: build
    cp target/release/pqview ~/.cargo/bin/

clean:
    cargo clean

check:
    cargo check

test:
    cargo test
