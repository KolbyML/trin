use alloy_rlp::Encodable;
use bytes::BytesMut;
use ethereum_types::Address;
use reth_primitives::Withdrawal as RethWithdrawal;
use rlp_derive::{RlpDecodable, RlpEncodable};
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, RlpDecodable, RlpEncodable)]
#[serde(rename_all = "camelCase")]
pub struct Withdrawal {
    #[serde(deserialize_with = "string_to_u64")]
    pub index: u64,
    #[serde(deserialize_with = "string_to_u64")]
    pub validator_index: u64,
    pub address: Address,
    #[serde(deserialize_with = "string_to_u64")]
    pub amount: u64,
}

impl From<RethWithdrawal> for Withdrawal {
    fn from(withdrawal: RethWithdrawal) -> Self {
        let mut buf = BytesMut::new();
        withdrawal.encode(&mut buf);
        rlp::decode(&buf)
            .expect("Both Reth and Portal header RLP formats should match so this shouldn't fail")
    }
}

fn string_to_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    u64::from_str_radix(s.trim_start_matches("0x"), 16)
        .map_err(|_| serde::de::Error::custom("failed to parse hex string"))
}
