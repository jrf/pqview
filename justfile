default:
    @just --list

build:
    cargo build --release

install: build
    #!/usr/bin/env sh
    if [ "$(uname)" = "Darwin" ]; then
        codesign --force --sign - target/release/pqview
    fi
    install -m 755 target/release/pqview ~/.cargo/bin/pqview.next
    mv -f ~/.cargo/bin/pqview.next ~/.cargo/bin/pqview

clean:
    cargo clean

check:
    cargo check

fmt:
    cargo fmt --all -- --check

lint:
    cargo clippy --all-targets --all-features -- -D warnings

test:
    cargo test

check-all: fmt check lint test
