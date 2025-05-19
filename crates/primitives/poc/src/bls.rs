#![allow(dead_code, unused_variables)]

use anyhow::{anyhow, bail, Error, Result};
use blst::min_pk::{
    AggregatePublicKey, AggregateSignature as BlstAggregateSignature, PublicKey, SecretKey,
    Signature as BlstSignature,
};
use serde::de;
use std::{
    fmt::{self, Display},
    sync::Arc,
};

use rand::Rng;
use serde::{de::Visitor, Deserialize, Serialize};
pub const HASH_DISPLAY_SIZE: usize = 3;

#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub struct AggregateSignature {
    pub signature: Signature,
    pub validators: Vec<ValidatorPublicKey>,
}

pub fn aggregate_public_key(validators: &[ValidatorPublicKey]) -> Result<PublicKey> {
    let pks = validators
        .iter()
        .map(|v| {
            PublicKey::uncompress(v.0.as_slice())
                .map_err(|e| anyhow!("Could not parse PublicKey: {:?}", e))
        })
        .collect::<Result<Vec<PublicKey>>>()?;

    let pks_refs: Vec<&PublicKey> = pks.iter().collect();

    let pk = AggregatePublicKey::aggregate(pks_refs.as_slice(), true)
        .map_err(|e| anyhow!("could not aggregate public keys: {:?}", e))?;

    Ok(pk.to_public_key())
}

impl AggregateSignature {
    pub fn aggregate_public_key(&self) -> Result<PublicKey> {
        aggregate_public_key(&self.validators)
    }

    pub fn aggregate_signature(&self) -> Result<BlstSignature> {
        let sig = BlstSignature::uncompress(&self.signature.0)
            .map_err(|e| anyhow!("Could not parse Signature: {:?}", e))?;
        Ok(sig)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub struct Signed<T> {
    pub msg: Vec<u8>,
    pub signature: T,
}

impl From<BlstSignature> for Signature {
    fn from(sig: BlstSignature) -> Self {
        Signature(sig.compress().as_slice().to_vec())
    }
}

impl fmt::Debug for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Signature")
            .field(&hex::encode(&self.0))
            .finish()
    }
}

impl fmt::Display for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            &hex::encode(self.0.get(..HASH_DISPLAY_SIZE).unwrap_or(&self.0))
        )
    }
}

impl Display for SignedByValidator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        _ = write!(f, " --> from validator {}", self.signature.validator);
        write!(f, "")
    }
}

#[derive(Clone, Default, PartialEq, Eq, Hash)]
pub struct Signature(pub Vec<u8>);

impl Serialize for Signature {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&format!("0x{}", hex::encode(&self.0)))
    }
}

impl<'de> Deserialize<'de> for Signature {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct SignatureVisitor;

        impl<'de> serde::de::Visitor<'de> for SignatureVisitor {
            type Value = Signature;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter
                    .write_str("a hex string representing a Signature (with optional 0x prefix)")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let value = value.trim_start_matches("0x");
                let bytes = hex::decode(value).map_err(de::Error::custom)?;
                Ok(Signature(bytes))
            }
        }

        deserializer.deserialize_str(SignatureVisitor)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub struct ValidatorSignature {
    pub signature: Signature,
    pub validator: ValidatorPublicKey,
}
pub type SignedByValidator = Signed<ValidatorSignature>;
pub type SignedAggregate = Signed<AggregateSignature>;

#[derive(Clone, Default, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub struct ValidatorPublicKey(pub Vec<u8>);

impl Serialize for ValidatorPublicKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(hex::encode(&self.0).as_str())
    }
}

impl<'de> Deserialize<'de> for ValidatorPublicKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct ValidatorPublicKeyVisitor;

        impl Visitor<'_> for ValidatorPublicKeyVisitor {
            type Value = ValidatorPublicKey;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a hex string representing a ValidatorPublicKey")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let bytes = hex::decode(value).map_err(de::Error::custom)?;
                Ok(ValidatorPublicKey(bytes))
            }
        }

        deserializer.deserialize_str(ValidatorPublicKeyVisitor)
    }
}

impl std::fmt::Debug for ValidatorPublicKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ValidatorPubK")
            .field(&hex::encode(
                self.0.get(..HASH_DISPLAY_SIZE).unwrap_or(&self.0),
            ))
            .finish()
    }
}

impl std::fmt::Display for ValidatorPublicKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            &hex::encode(self.0.get(..HASH_DISPLAY_SIZE).unwrap_or(&self.0),)
        )
    }
}

#[derive(Clone, Debug)]
pub struct BlstCrypto {
    sk: SecretKey,
    validator_pubkey: ValidatorPublicKey,
}

pub type SharedBlstCrypto = Arc<BlstCrypto>;

#[derive(Default)]
struct Aggregates {
    sigs: Vec<BlstSignature>,
    pks: Vec<PublicKey>,
    val: Vec<ValidatorPublicKey>,
}

const DST: &[u8] = b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_NUL_";
pub const SIG_SIZE: usize = 48;

impl BlstCrypto {
    pub fn new(validator_name: String) -> Result<Self> {
        // TODO load secret key from keyring or other
        // here basically secret_key <=> validator_id which is very badly secure !
        let validator_name_bytes = validator_name.as_bytes();
        let mut ikm = [0u8; 32];
        let len = std::cmp::min(validator_name_bytes.len(), 32);
        #[allow(clippy::indexing_slicing, reason = "len checked")]
        ikm[..len].copy_from_slice(&validator_name_bytes[..len]);

        let sk = SecretKey::key_gen(&ikm, &[])
            .map_err(|e| anyhow!("Could not generate key: {:?}", e))?;
        let validator_pubkey = as_validator_pubkey(sk.sk_to_pk());

        Ok(BlstCrypto {
            sk,
            validator_pubkey,
        })
    }

    pub fn new_random() -> Result<Self> {
        let mut rng = rand::rng();
        let id: String = (0..32)
            .map(|_| rng.random_range(33..127) as u8 as char) // Caractères imprimables ASCII
            .collect();
        Self::new(id.as_str().into())
    }

    pub fn validator_pubkey(&self) -> &ValidatorPublicKey {
        &self.validator_pubkey
    }

    pub fn sign(&self, msg: &[u8]) -> Result<SignedByValidator, Error> {
        let signature = self.sign_bytes(msg).into();
        Ok(SignedByValidator {
            msg: msg.to_vec(),
            signature: ValidatorSignature {
                signature,
                validator: self.validator_pubkey.clone(),
            },
        })
    }

    pub fn verify(msg: &SignedByValidator) -> Result<bool, Error> {
        let pk = PublicKey::uncompress(&msg.signature.validator.0)
            .map_err(|e| anyhow!("Could not parse PublicKey: {:?}", e))?;
        let sig = BlstSignature::uncompress(&msg.signature.signature.0)
            .map_err(|e| anyhow!("Could not parse Signature: {:?}", e))?;
        Ok(BlstCrypto::verify_bytes(&msg.msg, &sig, &pk))
    }

    pub fn verify_aggregate(msg: &Signed<AggregateSignature>) -> Result<bool, Error> {
        let pk = msg.signature.aggregate_public_key()?;
        let sig = BlstSignature::uncompress(&msg.signature.signature.0)
            .map_err(|e| anyhow!("Could not parse Signature: {:?}", e))?;
        Ok(BlstCrypto::verify_bytes(&msg.msg, &sig, &pk))
    }

    pub fn sign_aggregate(
        &self,
        msg: &[u8],
        aggregates: &[SignedByValidator],
    ) -> Result<Signed<AggregateSignature>, Error> {
        let self_signed = self.sign(msg)?;
        Self::aggregate(msg, &[aggregates, &[self_signed]].concat())
    }

    pub fn aggregate(
        msg: &[u8],
        aggregates: &[SignedByValidator],
    ) -> Result<Signed<AggregateSignature>, Error> {
        match aggregates.len() {
            0 => bail!("No signatures to aggregate"),
            1 => Ok(Signed {
                msg: msg.to_vec(),
                signature: AggregateSignature {
                    signature: aggregates[0].signature.signature.clone(),
                    validators: vec![aggregates[0].signature.validator.clone()],
                },
            }),
            _ => {
                let Aggregates { sigs, pks, val } = Self::extract_aggregates(aggregates)?;

                let pks_refs: Vec<&PublicKey> = pks.iter().collect();
                let sigs_refs: Vec<&BlstSignature> = sigs.iter().collect();

                let aggregated_pk = AggregatePublicKey::aggregate(&pks_refs, true)
                    .map_err(|e| anyhow!("could not aggregate public keys: {:?}", e))?;

                let aggregated_sig = BlstAggregateSignature::aggregate(&sigs_refs, true)
                    .map_err(|e| anyhow!("could not aggregate signatures: {:?}", e))?;

                let valid = Self::verify_aggregate(&Signed {
                    msg: msg.to_vec(),
                    signature: AggregateSignature {
                        signature: aggregated_sig.to_signature().into(),
                        validators: vec![as_validator_pubkey(aggregated_pk.to_public_key())],
                    },
                })
                .map_err(|e| anyhow!("Failed for verify new aggregated signature! Reason: {e}"))?;

                if !valid {
                    return Err(anyhow!(
                        "Failed to aggregate signatures into valid one. Messages might be different."
                    ));
                }

                Ok(Signed {
                    msg: msg.to_vec(),
                    signature: AggregateSignature {
                        signature: aggregated_sig.to_signature().into(),
                        validators: val,
                    },
                })
            }
        }
    }

    fn sign_bytes(&self, msg: &[u8]) -> BlstSignature {
        self.sk.sign(msg, DST, &[])
    }

    fn verify_bytes(msg: &[u8], sig: &BlstSignature, pk: &PublicKey) -> bool {
        let err = sig.verify(true, msg, DST, &[], pk, true);

        matches!(err, blst::BLST_ERROR::BLST_SUCCESS)
    }

    /// Given a list of signed messages, returns lists of signatures, public keys and
    /// validators.
    fn extract_aggregates(aggregates: &[SignedByValidator]) -> Result<Aggregates> {
        let mut accu = Aggregates::default();

        for s in aggregates {
            let sig = BlstSignature::uncompress(&s.signature.signature.0)
                .map_err(|_| anyhow!("Could not parse Signature"))?;
            let pk = PublicKey::uncompress(&s.signature.validator.0)
                .map_err(|_| anyhow!("Could not parse Public Key"))?;
            let val = s.signature.validator.clone();

            accu.sigs.push(sig);
            accu.pks.push(pk);
            accu.val.push(val);
        }

        Ok(accu)
    }
}

fn as_validator_pubkey(pk: PublicKey) -> ValidatorPublicKey {
    ValidatorPublicKey(pk.compress().as_slice().to_vec())
}

#[cfg(test)]
mod tests {

    #[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
    pub struct Hello {
        pub version: u16,
        pub validator_pubkey: ValidatorPublicKey,
        pub name: String,
        pub da_address: String,
    }

    #[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
    pub enum HandshakeNetMessage {
        Hello(Hello),
        Verack,
        Ping,
        Pong,
    }

    use super::*;
    #[test]
    fn test_sign_bytes() {
        let crypto = BlstCrypto::new_random().unwrap();
        let msg = b"hello";
        let sig = crypto.sign_bytes(msg);
        let valid = BlstCrypto::verify_bytes(msg, &sig, &crypto.sk.sk_to_pk());
        assert!(valid);
    }

    #[test]
    fn test_sign() {
        let crypto = BlstCrypto::new_random().unwrap();
        let pub_key = ValidatorPublicKey(crypto.sk.sk_to_pk().to_bytes().as_slice().to_vec());
        let msg = b"hello";
        let signed = crypto.sign(msg).unwrap();
        let valid = BlstCrypto::verify(&signed).unwrap();
        assert!(valid);
    }

    fn new_signed(msg: &[u8]) -> (SignedByValidator, ValidatorPublicKey) {
        let crypto = BlstCrypto::new_random().unwrap();
        let pub_key = crypto.validator_pubkey();
        (crypto.sign(msg).unwrap(), pub_key.clone())
    }

    #[test]
    fn test_sign_aggregate() {
        let (s1, pk1) = new_signed(b"hello");
        let (s2, pk2) = new_signed(b"hello");
        let (s3, pk3) = new_signed(b"hello");
        let (_, pk4) = new_signed(b"hello");

        let crypto = BlstCrypto::new_random().unwrap();
        let aggregates = vec![s1, s2, s3];
        let mut signed = crypto
            .sign_aggregate(b"hello", aggregates.as_slice())
            .unwrap();

        assert_eq!(
            signed.signature.validators,
            vec![
                pk1.clone(),
                pk2.clone(),
                pk3.clone(),
                crypto.validator_pubkey.clone(),
            ]
        );
        assert!(BlstCrypto::verify_aggregate(&signed).unwrap());

        // ordering should not matter
        signed.signature.validators = vec![
            pk2.clone(),
            pk1.clone(),
            pk3.clone(),
            crypto.validator_pubkey.clone(),
        ];
        assert!(BlstCrypto::verify_aggregate(&signed).unwrap());

        // Wrong validators
        signed.signature.validators = vec![
            pk1.clone(),
            pk2.clone(),
            pk4.clone(),
            crypto.validator_pubkey.clone(),
        ];
        assert!(!BlstCrypto::verify_aggregate(&signed).unwrap());

        // Wrong duplicated validators
        signed.signature.validators = vec![
            pk1.clone(),
            pk1.clone(),
            pk2.clone(),
            pk4.clone(),
            crypto.validator_pubkey.clone(),
        ];
        assert!(!BlstCrypto::verify_aggregate(&signed).unwrap());
    }

    #[test]
    fn test_sign_aggregate_wrong_message() {
        let msg = b"hello";
        let (s1, pk1) = new_signed(msg);
        let (s2, pk2) = new_signed(msg);
        let (s3, pk3) = new_signed(b"world"); // different message

        let crypto = BlstCrypto::new_random().unwrap();
        let aggregates = vec![s1, s2, s3];
        let signed = crypto.sign_aggregate(msg, aggregates.as_slice());

        assert!(signed.is_err_and(|e| {
            e.to_string()
                .contains("Failed to aggregate signatures into valid one.")
        }));
    }

    #[test]
    fn test_sign_aggregate_overlap() {
        let msg = b"hello";
        let (s1, pk1) = new_signed(msg);
        let (s2, pk2) = new_signed(msg);
        let (s3, pk3) = new_signed(msg);
        let (s4, pk4) = new_signed(msg);

        let crypto = BlstCrypto::new_random().unwrap();
        println!("crypto: {crypto:?}");
        let aggregates = vec![s1, s2.clone(), s3.clone(), s2, s3, s4];
        let signed = crypto.sign_aggregate(msg, aggregates.as_slice()).unwrap();
        assert!(BlstCrypto::verify_aggregate(&signed).unwrap());
        println!("Signed: {signed:?}");
        assert_eq!(
            signed.signature.validators,
            vec![
                pk1.clone(),
                pk2.clone(),
                pk3.clone(),
                pk2.clone(),
                pk3.clone(),
                pk4.clone(),
                crypto.validator_pubkey.clone(),
            ]
        );
    }
}
