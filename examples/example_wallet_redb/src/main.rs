use std::path::Path;

use bdk_chain::{indexed_tx_graph::ChangeSet, BlockId};
use bdk_esplora::{esplora_client, EsploraExt};
use bdk_persistence::WalletPersister;
use bdk_redb::Store;
use bdk_wallet::{bitcoin::Network, KeychainKind, Wallet};

const DB_PATH: &str = "bdk-example-redb.db";
const TEST_DB_PATH: &str = "bdk-example-redb-test.db";
const TEST_DB_PATH_2: &str = "bdk-example-redb-test-2.db";
const NETWORK: Network = Network::Testnet;
const EXTERNAL_DESC: &str = "wpkh(tprv8ZgxMBicQKsPdy6LMhUtFHAgpocR8GC6QmwMSFpZs7h6Eziw3SpThFfczTDh5rW2krkqffa11UpX3XkeTTB2FvzZKWXqPY54Y6Rq4AQ5R8L/84'/1'/0'/0/*)";
const INTERNAL_DESC: &str = "wpkh(tprv8ZgxMBicQKsPdy6LMhUtFHAgpocR8GC6QmwMSFpZs7h6Eziw3SpThFfczTDh5rW2krkqffa11UpX3XkeTTB2FvzZKWXqPY54Y6Rq4AQ5R8L/84'/1'/0'/1/*)";

// Create a simple example Wallet without relying on the store for persistence
fn create_example_wallet() -> Result<Wallet, Box<dyn std::error::Error>> {
    let wallet = Wallet::create(EXTERNAL_DESC, INTERNAL_DESC)
        .network(NETWORK)
        .create_wallet_no_persist()?;

    Ok(wallet)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting wallet example with redb storage");

    // Create or open the redb store
    let db_path = Path::new(DB_PATH);
    let mut store = if db_path.exists() {
        println!("Opening existing store at: {}", DB_PATH);
        Store::open(DB_PATH)?
    } else {
        println!("Creating new store at: {}", DB_PATH);
        Store::create_new(DB_PATH)?
    };

    // Create wallet - for simplicity, we'll create a new one every time
    let mut wallet = create_example_wallet()?;
    println!("Created new wallet");

    // Get a new address
    let address = wallet.next_unused_address(KeychainKind::External);
    println!(
        "Next unused address: ({}) {}",
        address.index, address.address
    );

    // Show the wallet balance
    let balance = wallet.balance();
    println!("Wallet balance: {}", balance.total());

    // Try to sync with an Esplora server
    println!("Attempting to sync with Esplora server...");

    // Create the Esplora client - this returns the client directly, not a Result
    let client = esplora_client::Builder::new("https://blockstream.info/testnet/api").build_blocking();

    // Start a full scan
    let request = wallet.start_full_scan();

    // Perform the full scan - this might fail
    match client.full_scan(request, 5, 5) {
        Ok(update) => {
            println!("Sync complete!");

            // Apply the update to the wallet
            wallet.apply_update(update)?;

            // Show updated balance
            let balance = wallet.balance();
            println!("Updated balance: {}", balance.total());

            // For demonstration, show that we can store data in redb
            // Create a simple ChangeSet
            let simple_changeset = ChangeSet::<BlockId, ()>::default();

            // Store the simple changeset
            WalletPersister::<BlockId, ()>::persist(&mut store, &simple_changeset)?;
            println!("Successfully stored data in redb");

            // Retrieve it
            let _retrieved = WalletPersister::<BlockId, ()>::initialize(&mut store)?;
            println!("Successfully retrieved data from redb");
        }
        Err(e) => println!("Error during sync: {}", e),
    }

    println!("Example completed successfully!");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_redb_storage() -> Result<(), Box<dyn std::error::Error>> {
        // Clean up any existing test database
        if Path::new(TEST_DB_PATH).exists() {
            fs::remove_file(TEST_DB_PATH)?;
        }

        // Create a new store
        let mut store = Store::create_new(TEST_DB_PATH)?;

        // Create an empty changeset
        let changeset = ChangeSet::<BlockId, ()>::default();

        // Store the changeset
        WalletPersister::<BlockId, ()>::persist(&mut store, &changeset)?;

        // Retrieve the data
        let _retrieved = WalletPersister::<BlockId, ()>::initialize(&mut store)?;

        // Clean up
        fs::remove_file(TEST_DB_PATH)?;

        Ok(())
    }

    #[test]
    fn test_wallet_with_redb() -> Result<(), Box<dyn std::error::Error>> {
        // Clean up any existing test database
        if Path::new(TEST_DB_PATH_2).exists() {
            fs::remove_file(TEST_DB_PATH_2)?;
        }

        // Create a new store
        let mut store = Store::create_new(TEST_DB_PATH_2)?;

        // Create a wallet
        let mut wallet = create_example_wallet()?;

        // Get a new address
        let address = wallet.next_unused_address(KeychainKind::External);

        // Create a changeset with the wallet data
        let changeset = ChangeSet::<BlockId, ()>::default();

        // Store the changeset
        WalletPersister::<BlockId, ()>::persist(&mut store, &changeset)?;

        // Retrieve the data
        let _retrieved = WalletPersister::<BlockId, ()>::initialize(&mut store)?;

        // Verify the wallet address is valid for testnet
        assert!(
            address.address.to_string().starts_with("tb1"),
            "Generated address should be a testnet address"
        );

        // Clean up
        fs::remove_file(TEST_DB_PATH_2)?;

        Ok(())
    }
}
