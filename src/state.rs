use std::{any::type_name, collections::HashMap};
use schemars::JsonSchema;
use secret_toolkit::serialization::{Bincode2, Serde};
use serde::{de::DeserializeOwned, Serialize, Deserialize};

use cosmwasm_std::{CanonicalAddr, ReadonlyStorage, StdError, StdResult, Storage, Uint128};

pub fn save<T: Serialize, S: Storage>(storage: &mut S, key: &[u8], value: &T) -> StdResult<()> {
    storage.set(key, &Bincode2::serialize(value)?);
    Ok(())
}

pub fn load<T: DeserializeOwned, S: ReadonlyStorage>(storage: &S, key: &[u8]) -> StdResult<T> {
    Bincode2::deserialize(
        &storage
            .get(key)
            .ok_or_else(|| StdError::not_found(type_name::<T>()))?,
    )
}

pub fn may_load<T: DeserializeOwned, S: ReadonlyStorage>(
    storage: &S,
    key: &[u8],
) -> StdResult<Option<T>> {
    match storage.get(key) {
        Some(value) => Bincode2::deserialize(&value).map(Some),
        None => Ok(None),
    }
}

pub fn remove<S: Storage>(storage: &mut S, key: &[u8]) {
    storage.remove(key);
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RoundStruct {
    pub round_number: u32,
    pub pool_size: Uint128,
    pub lucky_number: Option<i16>,
    pub users_count: u32,
    pub round_end_timestamp: Option<u64>,
    pub users_picked_numbers_count: Vec<u32>
}
  
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UserBetStruct {
    pub round_number: u32,
    pub tier: i8,
    pub number: i16,
    pub claimed_reward: bool,
    pub timestamp: u64
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UserBetsStruct {
    pub bet_keys: Vec<String>,
    pub bets: HashMap<String,UserBetStruct>,
}
