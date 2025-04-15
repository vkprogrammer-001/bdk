#![doc = include_str!("../README.md")]

use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;

use bdk_chain::{indexed_tx_graph::ChangeSet, Anchor, Merge};
use bdk_persistence::{AsyncWalletPersister, WalletPersister};
use redb::{
    CommitError, Database, DatabaseError, ReadableTable, StorageError, TableDefinition, TableError,
    TransactionError,
};
use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;

const CHANGESET_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("changesets");

#[derive(Error, Debug)]
pub enum RedbStoreError {
    #[error("Database error: {0}")]
    Database(#[from] redb::Error),
    #[error("Database error: {0}")]
    DatabaseError(#[from] DatabaseError),
    #[error("Transaction error: {0}")]
    TransactionError(#[from] TransactionError),
    #[error("Table error: {0}")]
    TableError(#[from] TableError),
    #[error("Storage error: {0}")]
    StorageError(#[from] StorageError),
    #[error("Commit error: {0}")]
    CommitError(#[from] CommitError),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// A store that persists wallet changesets using redb.
#[derive(Debug)]
pub struct Store {
    db: Arc<Database>,
}

impl Store {
    /// Create a new store at the given path.
    pub fn create_new<P: AsRef<Path>>(path: P) -> Result<Self, RedbStoreError> {
        let db = Database::create(path)?;

        // Create the table if it doesn't exist
        let write_txn = db.begin_write()?;
        write_txn.open_table(CHANGESET_TABLE)?;
        write_txn.commit()?;

        Ok(Self { db: Arc::new(db) })
    }

    /// Open an existing store at the given path.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, RedbStoreError> {
        let db = Database::open(path)?;

        // Ensure the table exists
        let write_txn = db.begin_write()?;
        write_txn.open_table(CHANGESET_TABLE)?;
        write_txn.commit()?;

        Ok(Self { db: Arc::new(db) })
    }

    /// Get the number of changesets stored.
    pub fn changeset_count(&self) -> Result<u64, RedbStoreError> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(CHANGESET_TABLE)?;
        Ok(table.len()? as u64)
    }

    /// Aggregate all changesets into a single changeset.
    pub fn aggregate_changesets<A, IA>(&self) -> Result<Option<ChangeSet<A, IA>>, RedbStoreError>
    where
        A: Serialize + DeserializeOwned + Ord + Anchor,
        IA: Serialize + DeserializeOwned + Default + Merge,
    {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(CHANGESET_TABLE)?;

        let mut aggregated = ChangeSet::default();
        let mut has_data = false;

        for entry in table.iter()? {
            let (_, value) = entry?;
            let value_slice = value.value();
            let changeset: ChangeSet<A, IA> = serde_json::from_slice(value_slice)?;
            aggregated.merge(changeset);
            has_data = true;
        }

        Ok(if has_data { Some(aggregated) } else { None })
    }

    /// Append a new changeset to the store.
    pub fn append_changeset<A, IA>(
        &self,
        changeset: &ChangeSet<A, IA>,
    ) -> Result<(), RedbStoreError>
    where
        A: Serialize + DeserializeOwned + Ord + Anchor,
        IA: Serialize + DeserializeOwned + Default + Merge,
    {
        if changeset.tx_graph.txs.is_empty() && changeset.indexer.is_empty() {
            return Ok(());
        }

        let write_txn = self.db.begin_write()?;

        // Use timestamp to ensure unique keys
        let key = format!(
            "changeset_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );

        // Serialize the changeset
        let serialized = serde_json::to_vec(changeset)?;

        {
            let mut table = write_txn.open_table(CHANGESET_TABLE)?;
            // Store it
            table.insert(key.as_str(), serialized.as_slice())?;
        } // table is dropped here, releasing the borrow

        write_txn.commit()?;

        Ok(())
    }
}

impl<A, IA> WalletPersister<A, IA> for Store
where
    A: Serialize + DeserializeOwned + Ord + Anchor,
    IA: Serialize + DeserializeOwned + Default + Merge,
{
    type Error = RedbStoreError;

    fn initialize(persister: &mut Self) -> Result<ChangeSet<A, IA>, Self::Error> {
        persister
            .aggregate_changesets()
            .map(Option::unwrap_or_default)
    }

    fn persist(persister: &mut Self, changeset: &ChangeSet<A, IA>) -> Result<(), Self::Error> {
        persister.append_changeset(changeset)
    }
}

impl<A, IA> AsyncWalletPersister<A, IA> for Store
where
    A: Serialize + DeserializeOwned + Ord + Anchor + Send + Sync,
    IA: Serialize + DeserializeOwned + Default + Merge + Send + Sync,
{
    type Error = RedbStoreError;

    fn initialize<'a>(
        persister: &'a mut Self,
    ) -> Pin<Box<dyn Future<Output = Result<ChangeSet<A, IA>, Self::Error>> + Send + 'a>>
    where
        Self: 'a,
    {
        Box::pin(async move {
            persister
                .aggregate_changesets()
                .map(Option::unwrap_or_default)
        })
    }

    fn persist<'a>(
        persister: &'a mut Self,
        changeset: &'a ChangeSet<A, IA>,
    ) -> Pin<Box<dyn Future<Output = Result<(), Self::Error>> + Send + 'a>>
    where
        Self: 'a,
    {
        Box::pin(async move { persister.append_changeset(changeset) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bdk_chain::bitcoin;
    use bdk_chain::BlockId;
    use std::sync::atomic::{AtomicU64, Ordering};
    use tempfile::tempdir;

    // Helper function to create a test transaction
    fn create_test_transaction() -> bitcoin::Transaction {
        bitcoin::Transaction {
            version: bitcoin::transaction::Version(1),
            lock_time: bitcoin::locktime::absolute::LockTime::ZERO,
            input: Vec::new(),
            output: Vec::new(),
        }
    }

    #[test]
    fn test_store_creation_and_retrieval() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.redb");

        let store = Store::create_new(&db_path).unwrap();

        // Create a test changeset with BlockId as anchor and () as indexer
        let mut changeset = ChangeSet::<BlockId, ()>::default();
        changeset
            .tx_graph
            .txs
            .insert(Arc::new(create_test_transaction()));

        // Store the changeset
        store.append_changeset(&changeset).unwrap();

        // Retrieve and verify
        let retrieved = store
            .aggregate_changesets::<BlockId, ()>()
            .unwrap()
            .unwrap();
        assert!(!retrieved.tx_graph.txs.is_empty());
    }

    #[test]
    fn test_wallet_persister_trait() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("wallet_persister.redb");

        let mut store = Store::create_new(&db_path).unwrap();

        // Initialize should return empty changeset for new store
        let init_changeset = WalletPersister::<BlockId, ()>::initialize(&mut store).unwrap();
        assert!(init_changeset.tx_graph.txs.is_empty());

        // Create and persist a changeset
        let mut changeset = ChangeSet::<BlockId, ()>::default();
        changeset
            .tx_graph
            .txs
            .insert(Arc::new(create_test_transaction()));

        WalletPersister::<BlockId, ()>::persist(&mut store, &changeset).unwrap();

        // Verify it was stored by initializing again
        let retrieved = WalletPersister::<BlockId, ()>::initialize(&mut store).unwrap();
        assert!(!retrieved.tx_graph.txs.is_empty());
    }

    #[tokio::test]
    async fn test_async_wallet_persister_trait() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("wallet_async.redb");

        let mut store = Store::create_new(&db_path).unwrap();

        // Initialize should return empty changeset for new store
        let init_changeset = AsyncWalletPersister::<BlockId, ()>::initialize(&mut store)
            .await
            .unwrap();
        assert!(init_changeset.tx_graph.txs.is_empty());

        // Create and persist a changeset
        let mut changeset = ChangeSet::<BlockId, ()>::default();
        changeset
            .tx_graph
            .txs
            .insert(Arc::new(create_test_transaction()));

        AsyncWalletPersister::<BlockId, ()>::persist(&mut store, &changeset)
            .await
            .unwrap();

        // Verify it was stored by initializing again
        let retrieved = AsyncWalletPersister::<BlockId, ()>::initialize(&mut store)
            .await
            .unwrap();
        assert!(!retrieved.tx_graph.txs.is_empty());
    }

    #[test]
    fn test_multiple_changesets() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("multiple_changesets.redb");

        let store = Store::create_new(&db_path).unwrap();

        // Create first transaction with version 1
        let tx1 = bitcoin::Transaction {
            version: bitcoin::transaction::Version(1),
            lock_time: bitcoin::locktime::absolute::LockTime::ZERO,
            input: Vec::new(),
            output: vec![bitcoin::TxOut {
                value: bitcoin::Amount::from_sat(1000),
                script_pubkey: bitcoin::ScriptBuf::new(),
            }],
        };

        // Create second transaction with version 2 to ensure it's different
        let tx2 = bitcoin::Transaction {
            version: bitcoin::transaction::Version(2),
            lock_time: bitcoin::locktime::absolute::LockTime::ZERO,
            input: Vec::new(),
            output: vec![bitcoin::TxOut {
                value: bitcoin::Amount::from_sat(2000),
                script_pubkey: bitcoin::ScriptBuf::new(),
            }],
        };

        // Create and store multiple changesets
        let mut changeset1 = ChangeSet::<BlockId, ()>::default();
        changeset1.tx_graph.txs.insert(Arc::new(tx1));
        store.append_changeset(&changeset1).unwrap();

        let mut changeset2 = ChangeSet::<BlockId, ()>::default();
        changeset2.tx_graph.txs.insert(Arc::new(tx2));
        store.append_changeset(&changeset2).unwrap();

        // Verify we have data stored
        assert!(store.changeset_count().unwrap() > 0);

        // Verify merged result has correct number of transactions
        let merged = store
            .aggregate_changesets::<BlockId, ()>()
            .unwrap()
            .unwrap();

        // Print details for debugging
        println!("Number of transactions: {}", merged.tx_graph.txs.len());
        for tx in &merged.tx_graph.txs {
            println!(
                "Transaction version: {:?}, amount: {:?}",
                tx.version,
                tx.output.get(0).map(|o| o.value)
            );
        }

        assert_eq!(merged.tx_graph.txs.len(), 2);
    }

    #[test]
    fn test_empty_changeset() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("empty_changeset.redb");

        let store = Store::create_new(&db_path).unwrap();

        // Empty changeset should not be stored
        let empty_changeset = ChangeSet::<BlockId, ()>::default();
        store.append_changeset(&empty_changeset).unwrap();

        // Count should be zero
        assert_eq!(store.changeset_count().unwrap(), 0);

        // Aggregate should return None
        assert!(store
            .aggregate_changesets::<BlockId, ()>()
            .unwrap()
            .is_none());
    }

    #[test]
    fn test_concurrent_access() {
        use std::thread;

        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("concurrent.redb");

        let store = Arc::new(Store::create_new(&db_path).unwrap());
        let counter = Arc::new(AtomicU64::new(0));

        // Create multiple threads that write to the store
        let mut handles = vec![];
        for _ in 0..5 {
            let store_clone = Arc::clone(&store);
            let counter_clone = Arc::clone(&counter);

            handles.push(thread::spawn(move || {
                // Use a unique value for each thread
                let thread_id = counter_clone.fetch_add(1, Ordering::SeqCst);

                // Create a transaction with a unique output value
                let tx = bitcoin::Transaction {
                    version: bitcoin::transaction::Version(1),
                    lock_time: bitcoin::locktime::absolute::LockTime::ZERO,
                    input: Vec::new(),
                    output: vec![bitcoin::TxOut {
                        value: bitcoin::Amount::from_sat(1000 + thread_id * 1000),
                        script_pubkey: bitcoin::ScriptBuf::new(),
                    }],
                };

                let mut changeset = ChangeSet::<BlockId, ()>::default();
                changeset.tx_graph.txs.insert(Arc::new(tx));
                store_clone.append_changeset(&changeset).unwrap();
            }));
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify data was stored
        assert!(store.changeset_count().unwrap() > 0);

        // Verify all transaction data is present
        let merged = store
            .aggregate_changesets::<BlockId, ()>()
            .unwrap()
            .unwrap();

        // Print details for debugging
        println!("Number of transactions: {}", merged.tx_graph.txs.len());
        let mut amounts = Vec::new();
        for tx in &merged.tx_graph.txs {
            if let Some(out) = tx.output.get(0) {
                amounts.push(out.value.to_sat());
                println!("Transaction amount: {}", out.value.to_sat());
            }
        }

        assert_eq!(merged.tx_graph.txs.len(), 5);
    }

    #[test]
    fn test_reopen_store() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("reopen.redb");

        // Create and write to store
        {
            let store = Store::create_new(&db_path).unwrap();
            let mut changeset = ChangeSet::<BlockId, ()>::default();
            changeset
                .tx_graph
                .txs
                .insert(Arc::new(create_test_transaction()));
            store.append_changeset(&changeset).unwrap();
        }

        // Reopen store and verify data
        let store = Store::open(&db_path).unwrap();
        let retrieved = store
            .aggregate_changesets::<BlockId, ()>()
            .unwrap()
            .unwrap();
        assert!(!retrieved.tx_graph.txs.is_empty());
    }

    #[test]
    fn test_error_handling() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("error.redb");

        let store = Store::create_new(&db_path).unwrap();

        // Test with invalid JSON data
        let invalid_data: &[u8] = b"invalid json data";
        let write_txn = store.db.begin_write().unwrap();

        {
            let mut table = write_txn.open_table(CHANGESET_TABLE).unwrap();
            table.insert("changeset_0", invalid_data).unwrap();
        } // table is dropped here

        write_txn.commit().unwrap();

        // Should return serialization error
        let result = store.aggregate_changesets::<BlockId, ()>();
        assert!(matches!(result, Err(RedbStoreError::Serialization(_))));
    }
}
