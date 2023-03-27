use core::convert::Infallible;

use alloc::collections::BTreeSet;
use bitcoin::{OutPoint, Transaction, TxOut};

use crate::{
    sparse_chain::ChainPosition,
    tx_graph::{Additions, TxGraph, TxInGraph},
    BlockAnchor, ChainOracle, FullTxOut, ObservedIn, TxIndex, TxIndexAdditions,
};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct TxInChain<'a, T, A> {
    pub observed_in: ObservedIn<&'a A>,
    pub tx: TxInGraph<'a, T, A>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct TxOutInChain<'a, I, A> {
    pub spk_index: &'a I,
    pub txout: FullTxOut<ObservedIn<&'a A>>,
}

pub struct IndexedAdditions<A, D> {
    pub graph_additions: Additions<A>,
    pub index_delta: D,
}

impl<A, D: Default> Default for IndexedAdditions<A, D> {
    fn default() -> Self {
        Self {
            graph_additions: Default::default(),
            index_delta: Default::default(),
        }
    }
}

impl<A: BlockAnchor, D: TxIndexAdditions> TxIndexAdditions for IndexedAdditions<A, D> {
    fn append_additions(&mut self, other: Self) {
        let Self {
            graph_additions,
            index_delta,
        } = other;
        self.graph_additions.append(graph_additions);
        self.index_delta.append_additions(index_delta);
    }
}

pub struct IndexedTxGraph<A, I> {
    graph: TxGraph<A>,
    index: I,
}

impl<A, I: Default> Default for IndexedTxGraph<A, I> {
    fn default() -> Self {
        Self {
            graph: Default::default(),
            index: Default::default(),
        }
    }
}

impl<A: BlockAnchor, I: TxIndex> IndexedTxGraph<A, I> {
    /// Get a reference of the internal transaction graph.
    pub fn graph(&self) -> &TxGraph<A> {
        &self.graph
    }

    /// Get a reference of the internal transaction index.
    pub fn index(&self) -> &I {
        &self.index
    }

    /// Insert a `txout` that exists in `outpoint` with the given `observation`.
    pub fn insert_txout(
        &mut self,
        outpoint: OutPoint,
        txout: &TxOut,
        observation: ObservedIn<A>,
    ) -> IndexedAdditions<A, I::Additions> {
        IndexedAdditions {
            graph_additions: {
                let mut graph_additions = self.graph.insert_txout(outpoint, txout.clone());
                graph_additions.append(match observation {
                    ObservedIn::Block(anchor) => self.graph.insert_anchor(outpoint.txid, anchor),
                    ObservedIn::Mempool(seen_at) => {
                        self.graph.insert_seen_at(outpoint.txid, seen_at)
                    }
                });
                graph_additions
            },
            index_delta: <I as TxIndex>::index_txout(&mut self.index, outpoint, txout),
        }
    }

    pub fn insert_tx(
        &mut self,
        tx: &Transaction,
        observation: ObservedIn<A>,
    ) -> IndexedAdditions<A, I::Additions> {
        let txid = tx.txid();
        IndexedAdditions {
            graph_additions: {
                let mut graph_additions = self.graph.insert_tx(tx.clone());
                graph_additions.append(match observation {
                    ObservedIn::Block(anchor) => self.graph.insert_anchor(txid, anchor),
                    ObservedIn::Mempool(seen_at) => self.graph.insert_seen_at(txid, seen_at),
                });
                graph_additions
            },
            index_delta: <I as TxIndex>::index_tx(&mut self.index, tx),
        }
    }

    pub fn filter_and_insert_txs<'t, T>(
        &mut self,
        txs: T,
        observation: ObservedIn<A>,
    ) -> IndexedAdditions<A, I::Additions>
    where
        T: Iterator<Item = &'t Transaction>,
    {
        txs.filter_map(|tx| {
            if self.index.is_tx_relevant(tx) {
                Some(self.insert_tx(tx, observation.clone()))
            } else {
                None
            }
        })
        .fold(IndexedAdditions::default(), |mut acc, other| {
            acc.append_additions(other);
            acc
        })
    }

    pub fn relevant_heights(&self) -> BTreeSet<u32> {
        self.graph.relevant_heights()
    }

    pub fn try_list_chain_txs<'a, C>(
        &'a self,
        chain: C,
    ) -> impl Iterator<Item = Result<TxInChain<'a, Transaction, A>, C::Error>>
    where
        C: ChainOracle + 'a,
    {
        self.graph
            .full_transactions()
            .filter(|tx| self.index.is_tx_relevant(tx))
            .filter_map(move |tx| {
                self.graph
                    .try_get_chain_position(&chain, tx.txid)
                    .map(|v| v.map(|observed_in| TxInChain { observed_in, tx }))
                    .transpose()
            })
    }

    pub fn list_chain_txs<'a, C>(
        &'a self,
        chain: C,
    ) -> impl Iterator<Item = TxInChain<'a, Transaction, A>>
    where
        C: ChainOracle<Error = Infallible> + 'a,
    {
        self.try_list_chain_txs(chain)
            .map(|r| r.expect("error is infallible"))
    }

    pub fn try_list_chain_txouts<'a, C>(
        &'a self,
        chain: C,
    ) -> impl Iterator<Item = Result<TxOutInChain<'a, I::SpkIndex, A>, C::Error>>
    where
        C: ChainOracle + 'a,
        ObservedIn<A>: ChainPosition,
    {
        self.index.relevant_txouts().iter().filter_map(
            move |(op, (spk_i, txout))| -> Option<Result<_, C::Error>> {
                let graph_tx = self.graph.get_tx(op.txid)?;

                let is_on_coinbase = graph_tx.is_coin_base();

                let chain_position = match self.graph.try_get_chain_position(&chain, op.txid) {
                    Ok(Some(observed_at)) => observed_at,
                    Ok(None) => return None,
                    Err(err) => return Some(Err(err)),
                };

                let spent_by = match self.graph.try_get_spend_in_chain(&chain, *op) {
                    Ok(spent_by) => spent_by,
                    Err(err) => return Some(Err(err)),
                };

                let full_txout = FullTxOut {
                    outpoint: *op,
                    txout: txout.clone(),
                    chain_position,
                    spent_by,
                    is_on_coinbase,
                };

                let txout_in_chain = TxOutInChain {
                    spk_index: spk_i,
                    txout: full_txout,
                };

                Some(Ok(txout_in_chain))
            },
        )
    }

    pub fn list_chain_txouts<'a, C>(
        &'a self,
        chain: C,
    ) -> impl Iterator<Item = TxOutInChain<'a, I::SpkIndex, A>>
    where
        C: ChainOracle<Error = Infallible> + 'a,
        ObservedIn<A>: ChainPosition,
    {
        self.try_list_chain_txouts(chain)
            .map(|r| r.expect("error in infallible"))
    }

    /// Return relevant unspents.
    pub fn try_list_chain_utxos<'a, C>(
        &'a self,
        chain: C,
    ) -> impl Iterator<Item = Result<TxOutInChain<'a, I::SpkIndex, A>, C::Error>>
    where
        C: ChainOracle + 'a,
        ObservedIn<A>: ChainPosition,
    {
        self.try_list_chain_txouts(chain)
            .filter(|r| !matches!(r, Ok(txo) if txo.txout.spent_by.is_none()))
    }

    pub fn list_chain_utxos<'a, C>(
        &'a self,
        chain: C,
    ) -> impl Iterator<Item = TxOutInChain<'a, I::SpkIndex, A>>
    where
        C: ChainOracle<Error = Infallible> + 'a,
        ObservedIn<A>: ChainPosition,
    {
        self.try_list_chain_utxos(chain)
            .map(|r| r.expect("error is infallible"))
    }
}
