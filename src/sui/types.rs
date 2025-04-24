use serde::{Serialize, Deserialize};
use std::fmt;
use std::str::FromStr;
use anyhow::Result;
use sui_sdk::types::base_types::SuiAddress as NativeSuiAddress;
use sui_sdk::types::crypto::SuiKeyPair;

// ObjectID
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ObjectID(String);

impl ObjectID {
    pub fn new(id: String) -> Self {
        Self(id)
    }
}

impl fmt::Display for ObjectID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for ObjectID {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        Ok(ObjectID::new(s.to_string()))
    }
}

// SuiAddress
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SuiAddress(String);

impl SuiAddress {
    pub fn new(addr: String) -> Self {
        Self(addr)
    }
}

impl fmt::Display for SuiAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for SuiAddress {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        Ok(SuiAddress::new(s.to_string()))
    }
}

impl From<&SuiKeyPair> for SuiAddress {
    fn from(keypair: &SuiKeyPair) -> Self {
        // Get the address from the keypair's public key
        let native_address = NativeSuiAddress::from(&keypair.public());
        SuiAddress::new(format!("0x{}", native_address))
    }
}

impl From<String> for SuiAddress {
    fn from(addr: String) -> Self {
        SuiAddress::new(addr)
    }
}

// Transaction data related types
#[derive(Debug, Clone)]
pub struct TransactionData {
    // Simplified for our purposes
    pub sender: SuiAddress,
    pub gas_object: ObjectID,
    pub payload: Vec<u8>,
}

impl TransactionData {
    pub fn new_move_call(
        sender: SuiAddress,
        _package: ObjectID,
        _module: String,
        _function: String,
        _type_args: Vec<String>,
        _args: Vec<Vec<u8>>,
        gas_object: ObjectID,
        _gas_budget: u64,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            sender,
            gas_object,
            payload: vec![],
        })
    }
}

// Transaction envelope
#[derive(Debug, Clone)]
pub struct Transaction {
    pub data: TransactionData,
    pub signatures: Vec<Signature>,
}

impl Transaction {
    pub fn from_data(data: TransactionData, signatures: Vec<Signature>) -> Self {
        Self {
            data,
            signatures,
        }
    }
}

// ObjectArg for shared objects
pub enum ObjectArg {
    SharedObject {
        id: ObjectID,
        initial_shared_version: u64,
        mutable: bool,
    },
    // Other variants omitted for simplicity
}

// CallArg for function arguments
pub enum CallArg {
    Object(ObjectArg),
    Pure(Vec<u8>),
    // Other variants omitted for simplicity
}

// Signature type
pub type Signature = Vec<u8>; 