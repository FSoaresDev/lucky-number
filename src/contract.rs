use cosmwasm_std::{Api, Binary, CosmosMsg, Env, Extern, HandleResponse, HumanAddr, InitResponse, Querier, QueryResult, StdError, StdResult, Storage, Uint128, WasmMsg, to_binary};
use cosmwasm_storage::{PrefixedStorage, ReadonlyPrefixedStorage};

use crate::{msg::{CountResponse, HandleMsg, InitMsg, QueryAnswer, QueryMsg, Snip20Msg}, state::{load, save}};

/*
    5 min Lucky Number =>  1 sSCRT => 1 - 5
    1h Lucky Number =>  5 sSCRT => 1 - 15
    12h Lucky Number =>  10 sSCRT => 1-30
*/
pub const CONFIG_DATA: &[u8] = b"config";
pub const ROUNDS_DATA: &[u8] = b"rounds";
pub const HISTORY_BETS: &[u8] = b"historybets";
pub const LUCKY_NUMBER_STATE_TIER_1: &[u8] = b"tier1";
pub const LUCKY_NUMBER_STATE_TIER_2: &[u8] = b"tier2";
pub const LUCKY_NUMBER_STATE_TIER_3: &[u8] = b"tier3";
pub const BLOCK_SIZE: usize = 256;

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    let mut config_data = PrefixedStorage::new(CONFIG_DATA, &mut deps.storage);
    save(&mut config_data, b"owner", &deps.api.canonical_address(&env.message.sender)?)?;
    save(&mut config_data, b"triggerer", &deps.api.canonical_address(&msg.triggerer_address)?)?;
    save(&mut config_data, b"token_address", &deps.api.canonical_address(&msg.token_address)?)?;
    save(&mut config_data, b"token_hash", &msg.token_hash)?;

    let mut tier1_state = PrefixedStorage::new(LUCKY_NUMBER_STATE_TIER_1, &mut deps.storage);
    save(&mut tier1_state, b"entry_fee", &msg.tier1_entry_fee)?;
    save(&mut tier1_state, b"triggerer_fee", &msg.tier1_triggerer_fee)?;
    save(&mut tier1_state, b"min_entries", &msg.tier1_min_entries)?;
    save(&mut tier1_state, b"max_rand_number", &msg.tier1_max_rand_number)?;
    save(&mut tier1_state, b"pool_size", &0)?;
    save(&mut tier1_state, b"current_round", &0)?;

    let mut tier2_state = PrefixedStorage::new(LUCKY_NUMBER_STATE_TIER_2, &mut deps.storage);
    save(&mut tier2_state, b"entry_fee", &msg.tier2_entry_fee)?;
    save(&mut tier2_state, b"triggerer_fee", &msg.tier2_triggerer_fee)?;
    save(&mut tier2_state, b"min_entries", &msg.tier2_min_entries)?;
    save(&mut tier2_state, b"max_rand_number", &msg.tier2_max_rand_number)?;
    save(&mut tier2_state, b"pool_size", &0)?;
    save(&mut tier2_state, b"current_round", &0)?;

    let mut tier3_state = PrefixedStorage::new(LUCKY_NUMBER_STATE_TIER_3, &mut deps.storage);
    save(&mut tier3_state, b"entry_fee", &msg.tier3_entry_fee)?;
    save(&mut tier3_state, b"triggerer_fee", &msg.tier3_triggerer_fee)?;
    save(&mut tier3_state, b"min_entries", &msg.tier3_min_entries)?;
    save(&mut tier3_state, b"max_rand_number", &msg.tier3_max_rand_number)?;
    save(&mut tier3_state, b"pool_size", &0)?;
    save(&mut tier3_state, b"current_round", &0)?;

    let snip20_register_msg = to_binary(&Snip20Msg::register_receive(env.clone().contract_code_hash))?;

    let token_response: Option<CosmosMsg> = Some(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: msg.token_address,
        callback_code_hash: msg.token_hash,
        msg: snip20_register_msg,
        send: vec![],
    }));

    Ok(InitResponse {
        messages: vec![token_response.unwrap()],
        log: vec![],
    })
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> StdResult<HandleResponse> {
    match msg {
        HandleMsg::Receive { sender, from, amount, msg } => try_receive(deps, env, sender, from, amount, msg),
        HandleMsg::ChangeTriggerer { triggerer } => try_change_triggerer(deps, env, triggerer)
    }
}

pub fn try_receive<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    _sender: HumanAddr,
    from: HumanAddr,
    amount: Uint128,
    msg: Option<Binary>,
) -> StdResult<HandleResponse> {
    Ok(HandleResponse::default())
}

pub fn try_change_triggerer<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    triggerer: HumanAddr,
) -> StdResult<HandleResponse> {
    Ok(HandleResponse::default())
}

pub fn query<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetTriggerer {} => to_binary(&query_triggerer(deps)?),
    }
}

fn query_triggerer<S: Storage, A: Api, Q: Querier>(deps: &Extern<S, A, Q>) -> QueryResult  {
    let config_data = ReadonlyPrefixedStorage::new(CONFIG_DATA, &deps.storage);
    let triggerer_address: HumanAddr = load(&config_data, b"triggerer").unwrap();

    to_binary(&QueryAnswer::GetTriggerer {
        triggerer: triggerer_address
    })
}

/* 
#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env};
    use cosmwasm_std::{coins, from_binary, StdError};

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies(20, &[]);

        let msg = InitMsg { count: 17 };
        let env = mock_env("creator", &coins(1000, "earth"));

        // we can just call .unwrap() to assert this was a success
        let res = init(&mut deps, env, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let res = query(&deps, QueryMsg::GetCount {}).unwrap();
        let value: CountResponse = from_binary(&res).unwrap();
        assert_eq!(17, value.count);
    }

    #[test]
    fn increment() {
        let mut deps = mock_dependencies(20, &coins(2, "token"));

        let msg = InitMsg { count: 17 };
        let env = mock_env("creator", &coins(2, "token"));
        let _res = init(&mut deps, env, msg).unwrap();

        // anyone can increment
        let env = mock_env("anyone", &coins(2, "token"));
        let msg = HandleMsg::Increment {};
        let _res = handle(&mut deps, env, msg).unwrap();

        // should increase counter by 1
        let res = query(&deps, QueryMsg::GetCount {}).unwrap();
        let value: CountResponse = from_binary(&res).unwrap();
        assert_eq!(18, value.count);
    }

    #[test]
    fn reset() {
        let mut deps = mock_dependencies(20, &coins(2, "token"));

        let msg = InitMsg { count: 17 };
        let env = mock_env("creator", &coins(2, "token"));
        let _res = init(&mut deps, env, msg).unwrap();

        // not anyone can reset
        let unauth_env = mock_env("anyone", &coins(2, "token"));
        let msg = HandleMsg::Reset { count: 5 };
        let res = handle(&mut deps, unauth_env, msg);
        match res {
            Err(StdError::Unauthorized { .. }) => {}
            _ => panic!("Must return unauthorized error"),
        }

        // only the original creator can reset the counter
        let auth_env = mock_env("creator", &coins(2, "token"));
        let msg = HandleMsg::Reset { count: 5 };
        let _res = handle(&mut deps, auth_env, msg).unwrap();

        // should now be 5
        let res = query(&deps, QueryMsg::GetCount {}).unwrap();
        let value: CountResponse = from_binary(&res).unwrap();
        assert_eq!(5, value.count);
    }
}
*/
