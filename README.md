# eccat

[![Rust](https://github.com/skycloudd/eccat/actions/workflows/rust.yml/badge.svg)](https://github.com/skycloudd/eccat/actions/workflows/rust.yml)

---

Eccat also runs on Lichess! It can be found at https://lichess.org/@/kybot

[![lichess-rapid](https://lichess-shield.vercel.app/api?username=kybot&format=bullet)](https://lichess.org/@/kybot/perf/bullet)
[![lichess-rapid](https://lichess-shield.vercel.app/api?username=kybot&format=blitz)](https://lichess.org/@/kybot/perf/blitz)
[![lichess-rapid](https://lichess-shield.vercel.app/api?username=kybot&format=rapid)](https://lichess.org/@/kybot/perf/rapid)

## Building

Eccat is built in [Rust](https://www.rust-lang.org/)

To install rust, follow the instructions at https://rustup.rs/

For the best performance, build with the `full` profile and the `pext` feature

```sh
cargo build --profile full --features=pext
```

Some cpus may not support the `pext` feature, in which case you can build without it

```sh
cargo build --profile full
```

Or if you just want a quick build without optimizations

```sh
cargo build
```

## Acknowledgements

Much thanks to [@tissatussa](https://github.com/tissatussa) for reporting various issues, problematic positions, and helping with testing the engine! :)
