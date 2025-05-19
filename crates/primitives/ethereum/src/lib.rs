#![allow(missing_docs)]

mod receipt;
mod trie;

pub use eth_trie::TrieError;
pub use ethereum::{
    util, AccessListItem, Block, BlockV0, BlockV1, BlockV2, EIP1559Transaction,
    EIP1559TransactionMessage, EIP2930Transaction, EIP2930TransactionMessage, EnvelopedDecodable,
    EnvelopedEncodable, Header, LegacyTransaction, LegacyTransactionMessage, PartialHeader,
    TransactionAction, TransactionSignature, TransactionV0, TransactionV1, TransactionV2,
};
pub use receipt::{EIP1559ReceiptData, EIP2930ReceiptData, EIP658ReceiptData, Log, Receipt};
pub use trie::{calculate_root, generate_proof, order_generate_proof, order_verify_proof};

pub mod keccak {
    use ethereum_types::H256;
    use hash256_std_hasher::Hash256StdHasher;
    use hash_db::Hasher;
    use sha3::Digest;

    /// Do a keccak 256-bit hash and return result.
    pub fn keccak_256(data: &[u8]) -> [u8; 32] {
        sha3::Keccak256::digest(data).into()
    }

    /// Concrete implementation of Hasher using Keccak 256-bit hashes
    #[derive(Debug)]
    pub struct KeccakHasher;

    impl Hasher for KeccakHasher {
        type Out = H256;
        type StdHasher = Hash256StdHasher;
        const LENGTH: usize = 32;

        fn hash(x: &[u8]) -> Self::Out {
            keccak_256(x).into()
        }
    }
}
