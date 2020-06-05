use crate::{rpc_types::*, RpcFunctions};
use snarkos_consensus::{get_block_reward, ConsensusParameters, MemoryPool};
use snarkos_dpc::base_dpc::{
    instantiated::{Components, MerkleTreeLedger, Tx},
    record::DPCRecord,
};
use snarkos_errors::rpc::RpcError;
use snarkos_models::objects::Transaction;
use snarkos_network::{context::Context, process_transaction_internal};
use snarkos_objects::BlockHeaderHash;
use snarkos_utilities::{
    bytes::{FromBytes, ToBytes},
    to_bytes,
};

use chrono::Utc;
use snarkos_models::dpc::Record;
use std::sync::Arc;
use tokio::{runtime::Runtime, sync::Mutex};

/// Implements JSON-RPC HTTP endpoint functions for a node.
/// The constructor is given Arc::clone() copies of all needed node components.
pub struct RpcImpl {
    /// Blockchain database storage.
    storage: Arc<MerkleTreeLedger>,

    /// Network context held by the server.
    server_context: Arc<Context>,

    /// Consensus parameters generated from node config.
    consensus: ConsensusParameters,

    /// Handle to access the memory pool of transactions.
    memory_pool_lock: Arc<Mutex<MemoryPool<Tx>>>,
}

impl RpcImpl {
    pub fn new(
        storage: Arc<MerkleTreeLedger>,
        server_context: Arc<Context>,
        consensus: ConsensusParameters,
        memory_pool_lock: Arc<Mutex<MemoryPool<Tx>>>,
    ) -> Self {
        Self {
            storage,
            server_context,
            consensus,
            memory_pool_lock,
        }
    }
}

impl RpcFunctions for RpcImpl {
    /// Returns all stored information on a block hash.
    fn get_block(&self, block_hash_string: String) -> Result<BlockInfo, RpcError> {
        let block_hash = hex::decode(&block_hash_string)?;
        assert_eq!(block_hash.len(), 32);

        let block_header_hash = BlockHeaderHash::new(block_hash);
        let height = match self.storage.get_block_num(&block_header_hash) {
            Ok(block_num) => match self.storage.is_canon(&block_header_hash) {
                true => Some(block_num),
                false => None,
            },
            Err(_) => None,
        };

        let confirmations = match height {
            Some(block_height) => self.storage.get_latest_block_height() - block_height,
            None => 0,
        };

        if let Ok(block) = self.storage.get_block(&block_header_hash) {
            let mut transactions = vec![];

            for transaction in block.transactions.iter() {
                transactions.push(hex::encode(&transaction.transaction_id()?));
            }

            // TODO get block info for non-canon blocks
            Ok(BlockInfo {
                hash: block_hash_string,
                height,
                confirmations,
                size: block.serialize()?.len(),
                time: block.header.time,
                difficulty_target: block.header.difficulty_target,
                nonce: block.header.nonce,
                merkle_root: hex::encode(block.header.merkle_root_hash.0),
                transactions,
                previous_block_hash: hex::encode(block.header.previous_block_hash.0),
            })
        } else {
            Err(RpcError::InvalidBlockHash(block_hash_string))
        }
    }

    /// Returns latest block height + 1 to account for genesis block 0.
    fn get_block_count(&self) -> Result<u32, RpcError> {
        Ok(self.storage.get_block_count())
    }

    /// Returns the block hash of the head of the canonical chain.
    fn get_best_block_hash(&self) -> Result<String, RpcError> {
        let best_block_hash = self.storage.get_block_hash(self.storage.get_latest_block_height())?;

        Ok(hex::encode(&best_block_hash.0))
    }

    /// Returns the block hash of the index specified if it exists in the canonical chain.
    fn get_block_hash(&self, block_height: u32) -> Result<String, RpcError> {
        let block_hash = self.storage.get_block_hash(block_height)?;

        Ok(hex::encode(&block_hash.0))
    }

    /// Returns hex encoded bytes of a transaction from its transaction id.
    fn get_raw_transaction(&self, transaction_id: String) -> Result<String, RpcError> {
        Ok(hex::encode(
            &self.storage.get_transaction_bytes(&hex::decode(transaction_id)?)?,
        ))
    }

    /// Returns information about a transaction from a transaction id.
    fn get_transaction_info(&self, transaction_id: String) -> Result<TransactionInfo, RpcError> {
        let transaction_bytes = self.get_raw_transaction(transaction_id)?;
        self.decode_raw_transaction(transaction_bytes)
    }

    /// Returns information about a transaction from serialized transaction bytes.
    fn decode_raw_transaction(&self, transaction_bytes: String) -> Result<TransactionInfo, RpcError> {
        let transaction_bytes = hex::decode(transaction_bytes)?;
        let transaction = Tx::read(&transaction_bytes[..])?;

        let mut old_serial_numbers = vec![];

        for sn in transaction.old_serial_numbers() {
            old_serial_numbers.push(hex::encode(to_bytes![sn]?));
        }

        let mut new_commitments = vec![];

        for cm in transaction.new_commitments() {
            new_commitments.push(hex::encode(to_bytes![cm]?));
        }

        let memo = hex::encode(to_bytes![transaction.memorandum()]?);

        let mut signatures = vec![];
        for sig in &transaction.signatures {
            signatures.push(hex::encode(to_bytes![sig]?));
        }

        Ok(TransactionInfo {
            txid: hex::encode(&transaction.transaction_id()?),
            size: transaction_bytes.len(),
            old_serial_numbers,
            new_commitments,
            memo,
            digest: hex::encode(to_bytes![transaction.digest]?),
            inner_proof: hex::encode(to_bytes![transaction.inner_proof]?),
            outer_proof: hex::encode(to_bytes![transaction.outer_proof]?),
            predicate_commitment: hex::encode(to_bytes![transaction.predicate_commitment]?),
            local_data_commitment: hex::encode(to_bytes![transaction.local_data_commitment]?),
            value_balance: transaction.value_balance,
            signatures,
        })
    }

    /// Send raw transaction bytes to this node to be added into the mempool.
    /// If valid, the transaction will be stored and propagated to all peers.
    /// Returns the transaction id if valid.
    fn send_raw_transaction(&self, transaction_bytes: String) -> Result<String, RpcError> {
        let transaction_bytes = hex::decode(transaction_bytes)?;
        let transaction = Tx::read(&transaction_bytes[..])?;

        match self.storage.transcation_conflicts(&transaction) {
            Ok(_) => {
                Runtime::new()?.block_on(process_transaction_internal(
                    self.server_context.clone(),
                    self.storage.clone(),
                    self.memory_pool_lock.clone(),
                    to_bytes![transaction]?.to_vec(),
                    self.server_context.local_address,
                ))?;

                Ok(hex::encode(transaction.transaction_id()?))
            }
            Err(_) => Ok("Transaction contains spent records".into()),
        }
    }

    /// Returns information about a record from serialized record bytes.
    fn decode_record(&self, record_bytes: String) -> Result<RecordInfo, RpcError> {
        let record_bytes = hex::decode(record_bytes)?;
        let record = DPCRecord::<Components>::read(&record_bytes[..])?;

        let account_public_key = hex::encode(to_bytes![record.account_public_key()]?);
        let payload = RPCRecordPayload {
            payload: hex::encode(to_bytes![record.payload()]?),
        };
        let birth_predicate_repr = hex::encode(record.birth_predicate_repr());
        let death_predicate_repr = hex::encode(record.death_predicate_repr());
        let serial_number_nonce = hex::encode(to_bytes![record.serial_number_nonce()]?);
        let commitment = hex::encode(to_bytes![record.commitment()]?);
        let commitment_randomness = hex::encode(to_bytes![record.commitment_randomness()]?);

        Ok(RecordInfo {
            account_public_key,
            is_dummy: record.is_dummy(),
            value: record.value(),
            payload,
            birth_predicate_repr,
            death_predicate_repr,
            serial_number_nonce,
            commitment,
            commitment_randomness,
        })
    }

    /// Fetch the number of connected peers this node has.
    fn get_connection_count(&self) -> Result<usize, RpcError> {
        // Create a temporary tokio runtime to make an asynchronous function call
        let peer_book = Runtime::new()?.block_on(self.server_context.peer_book.read());

        Ok(peer_book.connected_total() as usize)
    }

    /// Returns this nodes connected peers.
    fn get_peer_info(&self) -> Result<PeerInfo, RpcError> {
        // Create a temporary tokio runtime to make an asynchronous function call
        let peer_book = Runtime::new()?.block_on(self.server_context.peer_book.read());

        let mut peers = vec![];

        for (peer, _last_seen) in &peer_book.get_connected() {
            peers.push(peer.clone());
        }

        Ok(PeerInfo { peers })
    }

    /// Returns the current mempool and consensus information known by this node.
    fn get_block_template(&self) -> Result<BlockTemplate, RpcError> {
        let block_height = self.storage.get_latest_block_height();
        let block = self.storage.get_block_from_block_num(block_height)?;

        let time = Utc::now().timestamp();

        let memory_pool = Runtime::new()?.block_on(self.memory_pool_lock.lock());
        let full_transactions = memory_pool.get_candidates(&self.storage, self.consensus.max_block_size)?;

        let transaction_strings = full_transactions.serialize_as_str()?;

        let coinbase_value = get_block_reward(block_height + 1) + full_transactions.calculate_transaction_fees();

        Ok(BlockTemplate {
            previous_block_hash: hex::encode(&block.header.get_hash().0),
            block_height: block_height + 1,
            time,
            difficulty_target: self.consensus.get_block_difficulty(&block.header, time),
            transactions: transaction_strings,
            coinbase_value,
        })
    }
}

impl RpcImpl {}
