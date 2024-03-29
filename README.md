<h1 align="center">🚧 Async Gate</h1>

This Rust library is an asynchronous "gate" that can be waited to be raised or lowered, as controlled by a corresponding "lever".

## 💻 Installation

This crate is [published to crates.io as `async-gate`](https://crates.io/crates/async-gate), so you can do

```sh
cargo add async-gate
```

to add it to your project's dependencies.

## 🛠 Usage

You probably don't want to use this if you aren't me; the code is clunky and only moderately documented and tested. You might benefit more from using a plain [`tokio::sync::watch` channel](https://docs.rs/tokio/1.32.0/tokio/sync/watch/index.html) with your own layer of logic on top.

## 😵 Help! I have a question

Create an issue and I'll try to help.

## 😡 Fix! There is something that needs improvement

Create an issue or pull request and I'll try to fix.

## 📄 License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE] or https://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT] or https://opensource.org/licenses/MIT)

at your option.

## 🙏 Attribution

This implementation is heavily borrowed from @EFanZh's contributions [in this Rust forum post](https://users.rust-lang.org/t/a-flag-type-that-supports-waiting-asynchronously/91108/6).

The idea is highly inspired by [Python's `asyncio.Event`](https://docs.python.org/3/library/asyncio-sync.html#asyncio.Event), but a gate can be waited for to become 'clear' too (not just 'set').

This library is implemented with [`Tokio`](https://tokio.rs/)'s [`watch` channel](https://docs.rs/tokio/1.32.0/tokio/sync/watch/index.html).

I also developed [`awaitable-bool`](https://github.com/babichjacob/awaitable-bool) right after making `async-gate`. That unifies changing the value of the bool and waiting for value changes into a single type (`AwaitableBool`). It is simpler than this crate. 

_This README was generated with ❤️ by [readme-md-generator](https://github.com/kefranabg/readme-md-generator)_
