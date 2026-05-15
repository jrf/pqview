default:
    @just --list

build:
    cargo build --release

install: build
    #!/usr/bin/env sh
    if [ "$(uname)" = "Darwin" ]; then
        codesign --force --sign - target/release/pqview
    fi
    cp target/release/pqview ~/.cargo/bin/

clean:
    cargo clean

check:
    cargo check

test:
    cargo test
