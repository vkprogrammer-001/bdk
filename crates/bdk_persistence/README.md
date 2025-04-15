# BDK Persistence

The Persistence Traits for the Bitcoin Dev Kit (BDK)

## Overview

The `bdk_persistence` crate defines traits for persisting wallet data in the Bitcoin Development Kit ecosystem. It provides interfaces for both synchronous and asynchronous persistence implementations.

## Features

- `WalletPersister`: A synchronous trait for persisting wallet changesets
- `AsyncWalletPersister`: An asynchronous trait for persisting wallet changesets
- Integration with BDK's chain data model

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
bdk_persistence = "0.1.0"
```

The crate provides two main traits:

- `WalletPersister`: Synchronous storage API
- `AsyncWalletPersister`: Asynchronous storage API with Future support

Implementations of these traits can be used with BDK to store wallet state across application sessions.

## License

Licensed under either of

* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option. 