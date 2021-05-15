use cosmwasm_std::{Binary, HumanAddr, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    pub triggerer_address: HumanAddr,
    pub token_address: HumanAddr,
    pub token_hash: String,
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
    ChangeTriggerer { triggerer: HumanAddr}
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    // GetCount returns the current count as a json-encoded number
    GetTriggerer {},
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
}