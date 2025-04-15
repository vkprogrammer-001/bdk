# BDK Redb

A redb-based persistence implementation for BDK.

This crate provides a `Store` type that implements the `WalletPersister` and `AsyncWalletPersister` traits from `bdk_persistence`, allowing you to persist wallet data using the redb database.

## Features

- Persistent storage of wallet changesets using redb
- Support for both synchronous and asynchronous operations
- Thread-safe implementation using Arc
- Efficient storage and retrieval of wallet data

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
bdk_redb = "0.1.0"
```

Basic usage example:

```rust
use bdk_chain::{indexed_tx_graph::ChangeSet, BlockId};
use bdk_persistence::WalletPersister;
use bdk_redb::Store;

// Create a new store
let mut store = Store::create_new("wallet.db").unwrap();

// Use it with a wallet
let changeset = ChangeSet::<BlockId, ()>::default();
store.append_changeset(&changeset).unwrap();

// Retrieve all data
let data = WalletPersister::<BlockId, ()>::initialize(&mut store).unwrap();
```

## License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option. 