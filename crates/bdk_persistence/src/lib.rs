use bdk_chain::{indexed_tx_graph::ChangeSet, Anchor, Merge};
use std::future::Future;
use std::pin::Pin;

/// Trait that persists wallet changesets.
///
/// For an async version, use [`AsyncWalletPersister`].
pub trait WalletPersister<A, IA>
where
    A: Anchor,
    IA: Merge,
{
    /// Error type of the persister.
    type Error;

    /// Initialize the `persister` and load all data.
    ///
    /// This is called to ensure the [`WalletPersister`] is initialized and returns all data in the `persister`.
    ///
    /// # Implementation Details
    ///
    /// The database schema of the `persister` (if any), should be initialized and migrated here.
    ///
    /// The implementation must return all data currently stored in the `persister`. If there is no
    /// data, return an empty changeset (using [`ChangeSet::default()`]).
    ///
    /// Error should only occur on database failure. Multiple calls to `initialize` should not
    /// error. Calling `initialize` inbetween calls to `persist` should not error.
    ///
    /// Calling [`persist`] before the `persister` is `initialize`d may error. However, some
    /// persister implementations may NOT require initialization at all (and not error).
    ///
    /// [`persist`]: WalletPersister::persist
    fn initialize(persister: &mut Self) -> Result<ChangeSet<A, IA>, Self::Error>;

    /// Persist the given `changeset` to the `persister`.
    ///
    /// This method can fail if the `persister` is not [`initialize`]d.
    ///
    /// [`initialize`]: WalletPersister::initialize
    fn persist(persister: &mut Self, changeset: &ChangeSet<A, IA>) -> Result<(), Self::Error>;
}

/// Async trait that persists wallet changesets.
///
/// For a blocking version, use [`WalletPersister`].
pub trait AsyncWalletPersister<A, IA>
where
    A: Anchor + Send + Sync,
    IA: Merge + Send + Sync,
{
    /// Error type of the persister.
    type Error;

    /// Initialize the `persister` and load all data.
    ///
    /// This is called to ensure the [`AsyncWalletPersister`] is initialized and returns all data in the `persister`.
    ///
    /// # Implementation Details
    ///
    /// The database schema of the `persister` (if any), should be initialized and migrated here.
    ///
    /// The implementation must return all data currently stored in the `persister`. If there is no
    /// data, return an empty changeset (using [`ChangeSet::default()`]).
    ///
    /// Error should only occur on database failure. Multiple calls to `initialize` should not
    /// error. Calling `initialize` inbetween calls to `persist` should not error.
    ///
    /// Calling [`persist`] before the `persister` is `initialize`d may error. However, some
    /// persister implementations may NOT require initialization at all (and not error).
    ///
    /// [`persist`]: AsyncWalletPersister::persist
    fn initialize<'a>(
        persister: &'a mut Self,
    ) -> Pin<Box<dyn Future<Output = Result<ChangeSet<A, IA>, Self::Error>> + Send + 'a>>
    where
        Self: 'a;

    /// Persist the given `changeset` to the `persister`.
    ///
    /// This method can fail if the `persister` is not [`initialize`]d.
    ///
    /// [`initialize`]: AsyncWalletPersister::initialize
    fn persist<'a>(
        persister: &'a mut Self,
        changeset: &'a ChangeSet<A, IA>,
    ) -> Pin<Box<dyn Future<Output = Result<(), Self::Error>> + Send + 'a>>
    where
        Self: 'a;
}
