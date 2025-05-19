use bls::Signature;
pub use blst::min_pk::PublicKey;
use ethereum_types::H256;
use serde::{Deserialize, Deserializer};
use sha3::Digest;
use std::fmt;

pub mod bls;

// 为 PublicKey 实现自定义序列化
mod public_key_serde {
    use super::*;
    use serde::de;
    use std::fmt;

    pub fn serialize<S>(public_key: &PublicKey, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let bytes = public_key.to_bytes();
        serializer.serialize_str(&format!("0x{}", hex::encode(&bytes)))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<PublicKey, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct PublicKeyVisitor;

        impl<'de> serde::de::Visitor<'de> for PublicKeyVisitor {
            type Value = PublicKey;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter
                    .write_str("a hex string representing a PublicKey (with optional 0x prefix)")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let value = value.trim_start_matches("0x");
                let bytes = hex::decode(value).map_err(de::Error::custom)?;
                PublicKey::uncompress(&bytes).map_err(|e| {
                    de::Error::custom(format!("Failed to uncompress PublicKey: {:?}", e))
                })
            }
        }

        deserializer.deserialize_str(PublicKeyVisitor)
    }
}

#[derive(Clone, Debug, serde::Serialize, Deserialize)]
pub struct PoC {
    pub aggregate_signature: Signature,
    #[serde(with = "public_key_serde")]
    pub aggregate_public_key: PublicKey,
    pub root: H256,
}

impl fmt::Display for PoC {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "aggregate_signature: {}\naggregate_public_key: {}\nroot: {}",
            &hex::encode(&self.aggregate_signature.0),
            &hex::encode(self.aggregate_public_key.to_bytes()),
            self.root
        )
    }
}

pub mod generator {
    use crate::keccak_256;
    use ethereum_types::H256;

    fn compute_hash(input: Vec<u8>, output: Vec<u8>) -> Vec<u8> {
        println!("Input: {:?}", hex::encode(&input));
        println!("Output: {:?}", hex::encode(&output));
        let concat = [input, output].concat();
        println!("concat: {:?}", hex::encode(&concat));
        let data = keccak_256(&concat).to_vec();
        println!("Data: {:?}", hex::encode(&data));
        data
    }

    pub fn generate_root(list: Vec<(Vec<u8>, Vec<u8>)>) -> Result<H256, mp_ethereum::TrieError> {
        if list.is_empty() {
            return Ok(H256::zero());
        }

        let list = list
            .into_iter()
            .map(|(input, output)| compute_hash(input, output))
            .collect::<Vec<_>>();
        if list.len() == 1 {
            return Ok(H256::from_slice(&list[0]));
        }
        mp_ethereum::calculate_root(&list)
    }
}

impl TryFrom<bls::SignedAggregate> for PoC {
    type Error = anyhow::Error;
    fn try_from(singed: bls::SignedAggregate) -> Result<Self, Self::Error> {
        let bls::SignedAggregate { msg, signature } = singed;
        let aggregate_public_key = signature.aggregate_public_key()?;
        let aggregate_signature = signature.signature;
        let root = H256::from_slice(&msg);
        Ok(PoC {
            aggregate_signature,
            aggregate_public_key,
            root,
        })
    }
}

/// Do a keccak 256-bit hash and return result.
pub fn keccak_256(data: &[u8]) -> [u8; 32] {
    sha3::Keccak256::digest(data).into()
}

pub mod mock {
    use crate::{
        bls::{aggregate_public_key, BlstCrypto, SignedAggregate, SignedByValidator},
        generator,
    };
    use anyhow::Result;
    use blst::min_pk::PublicKey;

    pub struct MockPoC {
        keys: Vec<BlstCrypto>,
    }

    impl MockPoC {
        pub fn new() -> Self {
            let alice = BlstCrypto::new("Alice".to_string()).unwrap();
            let bob = BlstCrypto::new("Bob".to_string()).unwrap();
            let charlie = BlstCrypto::new("Charlie".to_string()).unwrap();
            Self {
                keys: vec![alice, bob, charlie],
            }
        }

        pub fn sign(&self, msg: &[u8]) -> Result<SignedByValidator> {
            self.keys[0].sign(msg)
        }

        pub fn sign_aggregate(&self, msg: &[u8]) -> Result<SignedAggregate> {
            let aggregate = self
                .keys
                .iter()
                .flat_map(|key| key.sign(msg))
                .collect::<Vec<_>>();
            BlstCrypto::aggregate(msg, aggregate.as_slice())
        }

        pub fn aggregate_public_key(&self) -> Result<PublicKey> {
            aggregate_public_key(
                &self
                    .keys
                    .iter()
                    .map(|key| key.validator_pubkey().clone())
                    .collect::<Vec<_>>(),
            )
        }

        pub fn generate_aggregate(&self, list: Vec<(Vec<u8>, Vec<u8>)>) -> Result<SignedAggregate> {
            let root = generator::generate_root(list)?;
            self.sign_aggregate(&root.to_fixed_bytes())
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::bls::{aggregate_public_key, BlstCrypto, SignedByValidator, ValidatorPublicKey};

    use super::*;

    fn new_signed(msg: &[u8]) -> (SignedByValidator, ValidatorPublicKey) {
        let crypto = BlstCrypto::new_random().unwrap();
        let pub_key = crypto.validator_pubkey();
        (crypto.sign(msg).unwrap(), pub_key.clone())
    }

    #[test]
    fn test_poc_verification() {
        let input = vec![(vec![1], vec![2])];
        let root = generator::generate_root(input).unwrap();
        let msg = root.to_fixed_bytes();

        let (signed, pub_key) = new_signed(&msg);
        let (signed1, pub_key1) = new_signed(&msg);
        let aggregates = vec![signed, signed1];
        let crypto = BlstCrypto::new_random().unwrap();
        let signed = crypto.sign_aggregate(&msg, aggregates.as_slice()).unwrap();
        println!(
            "Signed: {signed:?}, PubKey: {:?}",
            signed.signature.aggregate_public_key().unwrap().to_bytes()
        );
        let valid = BlstCrypto::verify_aggregate(&signed).unwrap();
        assert!(valid);
        let aggregate_pubkey =
            aggregate_public_key(&[pub_key1, pub_key, crypto.validator_pubkey().clone()]).unwrap();
        assert_eq!(
            signed.signature.aggregate_public_key().unwrap(),
            aggregate_pubkey
        );
        let poc = PoC::try_from(signed).unwrap();
        println!("PoC: {poc}");
        let serialized = serde_json::to_string(&poc).unwrap();
        println!("Serialized PoC: {serialized}");
    }

    #[test]
    fn test_mock_poc() {
        let mock = mock::MockPoC::new();
        let input = vec![(vec![1], vec![2])];
        let signed = mock.generate_aggregate(input).unwrap();
        let poc = PoC::try_from(signed).unwrap();
        println!("PoC: {poc}");
        let serialized = serde_json::to_string(&poc).unwrap();
        println!("Serialized PoC: {serialized}");
        println!(
            "{:?}",
            hex::encode(mock.aggregate_public_key().unwrap().to_bytes())
        );
    }
}
