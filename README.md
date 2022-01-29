# Siderite

A Minecraft 1.8.9 server written in Rust. (WIP)

## Building

To build siderite you'll need Rust and openssl-dev (possible package names: libssl-dev, openssl-devel) installed.
Rust can be installed using [rustup](https://rustup.rs/)

Once you have everything installed and cloned a local copy of siderite you can build it by running:
```sh
cd siderite
RUSTFLAGS="-C target-cpu=native" cargo build --release
```

The resulting binary can be found in `./target/release/`

To start the server simply run `./target/release/siderite`
