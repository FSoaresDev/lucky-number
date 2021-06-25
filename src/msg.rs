use cosmwasm_std::{Binary, HumanAddr, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::state::{RoundStruct, UserBetStruct, UserBetsStruct};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    pub entropy: u64,
    pub triggerer_address: HumanAddr,
    pub token_address: HumanAddr,
    pub token_hash: String,
    pub token_vk: String,
    pub tier1_entry_fee: Uint128,
    pub tier1_triggerer_fee: Uint128,
    pub tier1_min_entries: i16,
    pub tier1_max_rand_number: i16,
    pub tier2_entry_fee: Uint128,
    pub tier2_triggerer_fee: Uint128,
    pub tier2_min_entries: i16,
    pub tier2_max_rand_number: i16,
    pub tier3_entry_fee: Uint128,
    pub tier3_triggerer_fee: Uint128,
    pub tier3_min_entries: i16,
    pub tier3_max_rand_number: i16,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    Receive { sender: HumanAddr, from: HumanAddr, amount: Uint128, msg: Option<Binary> },
    Bet {tier: i8, number: i16},
    CreateViewingKey {
        entropy: String,
        padding: Option<String>,
    },
    SetViewingKey {
        key: String,
        padding: Option<String>,
    },
    Withdrawl {tier: i8, round: u32 },
    ChangeAdmin {admin: HumanAddr},
    ChangeTriggerer { triggerer: HumanAddr},
    ChangeTier { tier: i8, entry_fee: Uint128, triggerer_fee: Uint128, min_entries: i16, max_rand_number: i16 },
    TriggerLuckyNumber {tier1: bool, tier2: bool, tier3: bool, entropy: u64}
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    // GetCount returns the current count as a json-encoded number
    GetTriggerer {},
    GetUserBets {user_address: HumanAddr, viewing_key: String, keys: Vec<String>},
    GetPaginatedUserBets {user_address: HumanAddr, viewing_key: String, page: u32, page_size: u32},
    GetPaginatedRounds { tier1: bool, tier2: bool, tier3: bool, page: u32, page_size: u32},
    GetRounds { tier1_rounds: Vec<u32>, tier2_rounds: Vec<u32>, tier3_rounds: Vec<u32>},
    GetTierConfigs { tier1: bool, tier2: bool, tier3: bool},
    CheckTriggers {}
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct CountResponse {
}


// Messages sent to SNIP-20 contracts
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Snip20Msg {
    RegisterReceive {
        code_hash: String,
        padding: Option<String>,
    },
    Redeem {
        amount: Uint128,
        padding: Option<String>,
    },
}

impl Snip20Msg {
    pub fn register_receive(code_hash: String) -> Self {
        Snip20Msg::RegisterReceive {
            code_hash,
            padding: None, // TODO add padding calculation
        }
    }

    pub fn redeem(amount: Uint128) -> Self {
        Snip20Msg::Redeem {
            amount,
            padding: None, // TODO add padding calculation
        }
    }
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryAnswer {
    GetTriggerer {
        triggerer: HumanAddr
    },
    GetUserBets {
        user_bets: Vec<UserBetStruct>
    },
    GetPaginatedUserBets {
        user_bets: Vec<UserBetStruct>,
        bet_rounds: Vec<RoundStruct>,
        user_bets_total_count: usize
    },
    GetPaginatedRounds { 
        tier1_rounds: Option<Vec<RoundStruct>>,
        tier2_rounds: Option<Vec<RoundStruct>>,
        tier3_rounds: Option<Vec<RoundStruct>>
    },
    GetRounds {
        rounds: Vec<RoundStruct>
    },
    GetTierConfigs { 
        tier1_configs: Option<TierConfig>,
        tier2_configs: Option<TierConfig>,
        tier3_configs: Option<TierConfig>
    },
    CheckTriggers {
        tier1_trigger: bool,
        tier2_trigger: bool,
        tier3_trigger: bool,
    }
}

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleAnswer {
    /// generic status response
    Status {
        /// success or failure
        status: ResponseStatus,
        /// execution description
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },
    ViewingKey { key: String },
}
/// success or failure response
#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub enum ResponseStatus {
    Success,
    Failure,
}
#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct TierConfig {
    pub entry_fee: Uint128,
    pub triggerer_fee: Uint128,
    pub min_entries: i16,
    pub max_rand_number: i16
}