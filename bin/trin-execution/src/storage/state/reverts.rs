use revm::db::{
    states::{PlainStateReverts, PlainStorageRevert},
    RevertToSlot,
};
use revm_primitives::{AccountInfo, Address, Bytes, B256, U256};
use serde::Deserialize;
use ssz_derive::{Decode, Encode};

/// Struct to represent encodable/decodable None value for an SSZ enum
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct SszNone {
    // In rust, None is a variant not a type,
    // so we must use Option here to represent a None value
    pub value: Option<()>,
}

impl ssz::Decode for SszNone {
    fn is_ssz_fixed_len() -> bool {
        true
    }

    fn from_ssz_bytes(bytes: &[u8]) -> Result<Self, ssz::DecodeError> {
        match bytes.len() {
            0 => Ok(Self { value: None }),
            _ => Err(ssz::DecodeError::BytesInvalid(
                "Expected None value to be empty, found bytes.".to_string(),
            )),
        }
    }
}

impl ssz::Encode for SszNone {
    fn is_ssz_fixed_len() -> bool {
        true
    }

    fn ssz_append(&self, _buf: &mut Vec<u8>) {}

    fn ssz_bytes_len(&self) -> usize {
        0
    }
}

#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq)]
#[ssz(enum_behaviour = "transparent")]
pub enum SszRevertToSlot {
    Some(U256),
    Destroyed(SszNone),
}

impl SszRevertToSlot {
    pub fn to_previous_value(self) -> U256 {
        match self {
            SszRevertToSlot::Some(value) => value,
            SszRevertToSlot::Destroyed(_) => U256::ZERO,
        }
    }
}

impl From<RevertToSlot> for SszRevertToSlot {
    fn from(value: RevertToSlot) -> Self {
        match value {
            RevertToSlot::Some(value) => SszRevertToSlot::Some(value),
            RevertToSlot::Destroyed => SszRevertToSlot::Destroyed(SszNone::default()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default, Encode, Decode)]
pub struct SszPlainStorageRevert {
    pub address: Address,
    pub wiped: bool,
    pub storage_revert: Vec<(U256, SszRevertToSlot)>,
}

impl From<PlainStorageRevert> for SszPlainStorageRevert {
    fn from(value: PlainStorageRevert) -> Self {
        Self {
            address: value.address,
            wiped: value.wiped,
            storage_revert: value
                .storage_revert
                .into_iter()
                .map(|(key, value)| (key, value.into()))
                .collect(),
        }
    }
}

/// AccountInfo account information.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct SszAccountInfo {
    /// Account balance.
    pub balance: U256,
    /// Account nonce.
    pub nonce: u64,
    /// code hash,
    pub code_hash: B256,
    /// code: if None, `code_by_hash` will be used to fetch it if code needs to be loaded from
    /// inside `revm`.
    pub code: Option<Bytes>,
}

impl From<AccountInfo> for SszAccountInfo {
    fn from(value: AccountInfo) -> Self {
        Self {
            balance: value.balance,
            nonce: value.nonce,
            code_hash: value.code_hash,
            code: value.code.map(|bytecode| bytecode.original_bytes()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct Reverts {
    accounts: Vec<Vec<(Address, Option<SszAccountInfo>)>>,
    storage: Vec<Vec<SszPlainStorageRevert>>,
}

impl From<PlainStateReverts> for Reverts {
    fn from(value: PlainStateReverts) -> Self {
        Self {
            accounts: value
                .accounts
                .into_iter()
                .map(|accounts| {
                    accounts
                        .into_iter()
                        .map(|(address, account)| (address, account.map(|info| info.into())))
                        .collect()
                })
                .collect(),
            storage: value
                .storage
                .into_iter()
                .map(|storage| storage.into_iter().map(|revert| revert.into()).collect())
                .collect(),
        }
    }
}
