# Example: Wallet with redb Storage

This example demonstrates how to use the `bdk_redb` crate to persist wallet data using the redb database. It shows a complete wallet workflow including address generation, blockchain synchronization, and how to store/retrieve data from a redb store.

## Features Demonstrated

- Creating a redb store
- Creating a wallet with descriptors
- Generating addresses
- Syncing the wallet with blockchain (using Esplora)
- Checking balances
- Storing and retrieving data using the `WalletPersister` trait

## Running the Example

To run this example:

```bash
cargo run -p example_wallet_redb
```

## What it Does

1. Creates/opens a redb store at "bdk-example-redb.db"
2. Creates a new wallet with testnet descriptors
3. Generates a new address for receiving funds
4. Displays the wallet balance
5. Syncs the wallet with a Testnet Esplora server
6. Shows the updated balance after syncing
7. Demonstrates storing and retrieving data from the redb store

## Expected Output

```
Starting wallet example with redb storage
Opening existing store at: bdk-example-redb.db
Created new wallet
Next unused address: (0) tb1q...
Wallet balance: 0 BTC
Attempting to sync with Esplora server...
Sync complete!
Updated balance: 0.00001828 BTC
Successfully stored data in redb
Successfully retrieved data from redb
Example completed successfully!
```

## Notes

- This example uses the Testnet network
- The private keys are from test vectors and should not be used with real funds
- You can get free Testnet coins from a faucet to test the functionality
- This example demonstrates basic usage of redb storage but does not show the full integration of wallet state persistence 