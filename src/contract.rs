use std::collections::HashMap;

use cosmwasm_std::{Api, Binary, CanonicalAddr, CosmosMsg, Env, Extern, HandleResponse, HumanAddr, InitResponse, LogAttribute, Querier, QueryResult, StdError, StdResult, Storage, Uint128, WasmMsg, to_binary};
use cosmwasm_storage::{PrefixedStorage, ReadonlyPrefixedStorage};
use rand::Rng;
use rand_chacha::ChaChaRng;
use secret_toolkit::storage::{AppendStore, AppendStoreMut, TypedStore};
use sha2::{Digest, Sha256};
use rand_core::SeedableRng;
use crate::{msg::{CountResponse, HandleMsg, InitMsg, QueryAnswer, QueryMsg, Snip20Msg}, rand::sha_256, state::{RoundStruct, load, save}};

/*
    5 min Lucky Number =>  1 sSCRT => 1 - 5
    1h Lucky Number =>  5 sSCRT => 1 - 15
    12h Lucky Number =>  10 sSCRT => 1-30
*/
pub const CONFIG_DATA: &[u8] = b"config";
pub const LUCKY_NUMBER_CONFIG_TIER_1: &[u8] = b"tier1";
pub const LUCKY_NUMBER_CONFIG_TIER_2: &[u8] = b"tier2";
pub const LUCKY_NUMBER_CONFIG_TIER_3: &[u8] = b"tier3";
pub const HISTORY_BETS: &[u8] = b"historybets";

pub const ROUNDS_STATE: &[u8] = b"rounds";
pub const BLOCK_SIZE: usize = 256;

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    let prng_seed: Vec<u8> = sha_256(base64::encode(msg.entropy.clone().to_string()).as_bytes()).to_vec();
    let addition_entropy: Vec<u64> = Vec::new();

    let mut new_round: RoundStruct = RoundStruct {
        round_number: 0,
        lucky_number: None,
        users_count: 0,
        pool_size: Uint128(0),
        users_picked_numbers_count: vec![0; 0]
    };

    let mut config_data = PrefixedStorage::new(CONFIG_DATA, &mut deps.storage);
    save(&mut config_data, b"owner", &deps.api.canonical_address(&env.message.sender)?)?;
    save(&mut config_data, b"triggerer", &msg.triggerer_address)?;
    save(&mut config_data, b"token_address", &deps.api.canonical_address(&msg.token_address)?)?;
    save(&mut config_data, b"token_hash", &msg.token_hash)?;
    save(&mut config_data, b"entropy", &prng_seed)?;
    save(&mut config_data, b"base_entropy", &msg.entropy.clone().to_be_bytes())?;
    save(&mut config_data, b"addition_entropy", &addition_entropy)?;

    let mut tier1_state = PrefixedStorage::new(LUCKY_NUMBER_CONFIG_TIER_1, &mut deps.storage);
    save(&mut tier1_state, b"entry_fee", &msg.tier1_entry_fee)?;
    save(&mut tier1_state, b"triggerer_fee", &msg.tier1_triggerer_fee)?;
    save(&mut tier1_state, b"min_entries", &msg.tier1_min_entries)?;
    save(&mut tier1_state, b"max_rand_number", &msg.tier1_max_rand_number)?;
    let mut tier1_rounds = PrefixedStorage::multilevel(&[ROUNDS_STATE, &"tier1".to_string().as_bytes()], &mut deps.storage);
    let mut tier1_rounds_store: AppendStoreMut<RoundStruct, _> = AppendStoreMut::attach_or_create(&mut tier1_rounds)?;
    new_round.users_picked_numbers_count = vec![0; (&msg.tier1_max_rand_number + 1) as usize];
    tier1_rounds_store.push(&new_round)?;

    let mut tier2_state = PrefixedStorage::new(LUCKY_NUMBER_CONFIG_TIER_2, &mut deps.storage);
    save(&mut tier2_state, b"entry_fee", &msg.tier2_entry_fee)?;
    save(&mut tier2_state, b"triggerer_fee", &msg.tier2_triggerer_fee)?;
    save(&mut tier2_state, b"min_entries", &msg.tier2_min_entries)?;
    save(&mut tier2_state, b"max_rand_number", &msg.tier2_max_rand_number)?;
    let mut tier2_rounds = PrefixedStorage::multilevel(&[ROUNDS_STATE, &"tier2".to_string().as_bytes()], &mut deps.storage);
    let mut tier2_rounds_store: AppendStoreMut<RoundStruct, _> = AppendStoreMut::attach_or_create(&mut tier2_rounds)?;
    new_round.users_picked_numbers_count = vec![0; (&msg.tier2_max_rand_number + 1) as usize];
    tier2_rounds_store.push(&new_round)?;

    let mut tier3_state = PrefixedStorage::new(LUCKY_NUMBER_CONFIG_TIER_3, &mut deps.storage);
    save(&mut tier3_state, b"entry_fee", &msg.tier3_entry_fee)?;
    save(&mut tier3_state, b"triggerer_fee", &msg.tier3_triggerer_fee)?;
    save(&mut tier3_state, b"min_entries", &msg.tier3_min_entries)?;
    save(&mut tier3_state, b"max_rand_number", &msg.tier3_max_rand_number)?;
    let mut tier3_rounds = PrefixedStorage::multilevel(&[ROUNDS_STATE, &"tier3".to_string().as_bytes()], &mut deps.storage);
    let mut tier3_rounds_store: AppendStoreMut<RoundStruct, _> = AppendStoreMut::attach_or_create(&mut tier3_rounds)?;
    new_round.users_picked_numbers_count = vec![0; (&msg.tier3_max_rand_number + 1) as usize];
    tier3_rounds_store.push(&new_round)?;

    /*let snip20_register_msg = to_binary(&Snip20Msg::register_receive(env.clone().contract_code_hash))?;

    let token_response: Option<CosmosMsg> = Some(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: msg.token_address,
        callback_code_hash: msg.token_hash,
        msg: snip20_register_msg,
        send: vec![],
    }));
*/
    Ok(InitResponse {
        messages: vec![
           // token_response.unwrap()
        ],
        log: vec![],
    })
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> StdResult<HandleResponse> {
    match msg {
        // Bet
        HandleMsg::Receive { sender, from, amount, msg } => try_receive(deps, env, sender, from, amount, msg),
        HandleMsg::Withdrawl { tier, round } => try_withdrawl(deps, env, tier, round),

        // Triggerer
        HandleMsg::TriggerLuckyNumber { tier1, tier2, tier3, entropy } => try_trigger_lucky_number(deps, env, tier1, tier2, tier3, entropy),
        
        // Admin
        HandleMsg::ChangeTriggerer { triggerer } => try_change_triggerer(deps, env, triggerer),
        HandleMsg::ChangeTier { tier, entry_fee, triggerer_fee, min_entries, max_rand_number } => try_change_tier(deps, env, tier, entry_fee, triggerer_fee, min_entries, max_rand_number),
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

pub fn try_change_tier<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    tier: i8, 
    entry_fee: Uint128, 
    triggerer_fee: Uint128, 
    min_entries: i16, 
    max_rand_number: i16
) -> StdResult<HandleResponse> {
    Ok(HandleResponse::default())
}

pub fn try_withdrawl<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    tier: i8,
    round: i128
) -> StdResult<HandleResponse> {
    Ok(HandleResponse::default())
}

pub fn try_trigger_lucky_number<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    tier1: bool, 
    tier2: bool,  
    tier3: bool, 
    entropy: u64
) -> StdResult<HandleResponse> {
    // check if it is the triggerer
    
    let config_data = ReadonlyPrefixedStorage::new(CONFIG_DATA, &deps.storage);
    // Generate seed vector: original entropy + this request entropy + max 6 entropy stored from users
    let base_entropy = load(&config_data, b"base_entropy").unwrap();
    let mut addition_entropy: Vec<_> = load(&config_data, b"addition_entropy").unwrap();

    addition_entropy.push(base_entropy);
    addition_entropy.push(entropy.clone().to_be_bytes());

    let mut hasher = Sha256::new();
    addition_entropy.iter().for_each(|el| hasher.update(el));
    let seed:[u8; 32] = hasher.finalize().into();
    let mut rng = ChaChaRng::from_seed(seed);

    let mut lucky_number_tier_1: i16 = 0;
    let mut lucky_number_tier_2: i16 = 0;
    let mut lucky_number_tier_3: i16 = 0;

    if tier1 == true {
        let tier1_config = ReadonlyPrefixedStorage::new(LUCKY_NUMBER_CONFIG_TIER_1, &deps.storage);
        let min_entries_tier1: i16 = load(&tier1_config, b"min_entries").unwrap();
        let entry_fee_tier1: Uint128 = load(&tier1_config, b"entry_fee").unwrap();
        let max_rand_number_tier1: i16 = load(&tier1_config, b"max_rand_number").unwrap();

        let mut tier1_rounds = PrefixedStorage::multilevel(&[ROUNDS_STATE, &"tier1".to_string().as_bytes()], &mut deps.storage);
        let mut tier1_rounds_store: AppendStoreMut<RoundStruct, _> = AppendStoreMut::attach_or_create(&mut tier1_rounds)?;
        let tier1_cur_round: RoundStruct = tier1_rounds_store.get_at(tier1_rounds_store.len() - 1).unwrap();
        
        let pool_size_cur_round_tier1: Uint128 =  tier1_cur_round.pool_size;

        // check if there are enougth pool size (pool_size >= min_entries * entry_fee)
        if pool_size_cur_round_tier1 >= Uint128(min_entries_tier1 as u128).multiply_ratio(entry_fee_tier1, Uint128(1)) {
            lucky_number_tier_1 = rng.gen_range(1,max_rand_number_tier1);

            //update round
            let mut updated_round = tier1_cur_round;
            updated_round.lucky_number = Some(lucky_number_tier_1);
            tier1_rounds_store.set_at(tier1_rounds_store.len()-1,&updated_round);

            //new round
            let new_round: RoundStruct = RoundStruct {
                round_number: tier1_rounds_store.len(),
                lucky_number: None,
                users_count: 0,
                pool_size: Uint128(0),
                users_picked_numbers_count: vec![0; (max_rand_number_tier1 + 1) as usize]
            };
            tier1_rounds_store.push(&new_round)?;
        }
    }

    if tier2 == true {
        let tier2_config = ReadonlyPrefixedStorage::new(LUCKY_NUMBER_CONFIG_TIER_2, &deps.storage);
        let min_entries_tier2: i16 = load(&tier2_config, b"min_entries").unwrap();
        let entry_fee_tier2: Uint128 = load(&tier2_config, b"entry_fee").unwrap();
        let max_rand_number_tier2: i16 = load(&tier2_config, b"max_rand_number").unwrap();

        let mut tier2_rounds = PrefixedStorage::multilevel(&[ROUNDS_STATE, &"tier2".to_string().as_bytes()], &mut deps.storage);
        let mut tier2_rounds_store: AppendStoreMut<RoundStruct, _> = AppendStoreMut::attach_or_create(&mut tier2_rounds)?;
        let tier2_cur_round: RoundStruct = tier2_rounds_store.get_at(tier2_rounds_store.len() - 1).unwrap();
        
        let pool_size_cur_round_tier2: Uint128 =  tier2_cur_round.pool_size;

        // check if there are enougth pool size (pool_size >= min_entries * entry_fee)
        if pool_size_cur_round_tier2 >= Uint128(min_entries_tier2 as u128).multiply_ratio(entry_fee_tier2, Uint128(1)) {
            lucky_number_tier_2 = rng.gen_range(1,max_rand_number_tier2);

            //update round
            let mut updated_round = tier2_cur_round;
            updated_round.lucky_number = Some(lucky_number_tier_2);
            tier2_rounds_store.set_at(tier2_rounds_store.len()-1,&updated_round);

            //new round
            let new_round: RoundStruct = RoundStruct {
                round_number: tier2_rounds_store.len(),
                lucky_number: None,
                users_count: 0,
                pool_size: Uint128(0),
                users_picked_numbers_count: vec![0; (max_rand_number_tier2 + 1) as usize]
            };
            tier2_rounds_store.push(&new_round)?;
        }
    }

    if tier3 == true {
        let tier3_config = ReadonlyPrefixedStorage::new(LUCKY_NUMBER_CONFIG_TIER_3, &deps.storage);
        let min_entries_tier3: i16 = load(&tier3_config, b"min_entries").unwrap();
        let entry_fee_tier3: Uint128 = load(&tier3_config, b"entry_fee").unwrap();
        let max_rand_number_tier3: i16 = load(&tier3_config, b"max_rand_number").unwrap();

        let mut tier3_rounds = PrefixedStorage::multilevel(&[ROUNDS_STATE, &"tier3".to_string().as_bytes()], &mut deps.storage);
        let mut tier3_rounds_store: AppendStoreMut<RoundStruct, _> = AppendStoreMut::attach_or_create(&mut tier3_rounds)?;
        let tier3_cur_round: RoundStruct = tier3_rounds_store.get_at(tier3_rounds_store.len() - 1).unwrap();
        
        let pool_size_cur_round_tier3: Uint128 =  tier3_cur_round.pool_size;

        // check if there are enougth pool size (pool_size >= min_entries * entry_fee)
        if pool_size_cur_round_tier3 >= Uint128(min_entries_tier3 as u128).multiply_ratio(entry_fee_tier3, Uint128(1)) {
            lucky_number_tier_3 = rng.gen_range(1,max_rand_number_tier3);

            //update round
            let mut updated_round = tier3_cur_round;
            updated_round.lucky_number = Some(lucky_number_tier_3);
            tier3_rounds_store.set_at(tier3_rounds_store.len()-1,&updated_round);

            //new round
            let new_round: RoundStruct = RoundStruct {
                round_number: tier3_rounds_store.len(),
                lucky_number: None,
                users_count: 0,
                pool_size: Uint128(0),
                users_picked_numbers_count: vec![0; (max_rand_number_tier3 + 1) as usize]
            };
            tier3_rounds_store.push(&new_round)?;
        }
    }

    return Ok(HandleResponse {
        messages: vec![],
        log: vec![
            LogAttribute {key: "lucky_number_tier_1".to_string(), value: lucky_number_tier_1.to_string()},
            LogAttribute {key: "lucky_number_tier_2".to_string(), value: lucky_number_tier_2.to_string()},
            LogAttribute {key: "lucky_number_tier_3".to_string(), value: lucky_number_tier_3.to_string()},
        ],
        data: None,
    });
}

pub fn query<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetTriggerer {} => to_binary(&query_triggerer(deps)?),
        QueryMsg::GetRounds {tier1, tier2, tier3, page, page_size} => to_binary(&query_rounds(deps,tier1, tier2, tier3, page, page_size)?),
    }
}

fn query_triggerer<S: Storage, A: Api, Q: Querier>(deps: &Extern<S, A, Q>) -> QueryResult  {
    let config_data = ReadonlyPrefixedStorage::new(CONFIG_DATA, &deps.storage);
    let triggerer_address: HumanAddr = load(&config_data, b"triggerer").unwrap();

    to_binary(&QueryAnswer::GetTriggerer {
        triggerer: triggerer_address
    })
}

fn query_rounds<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    tier1: bool,
    tier2: bool,
    tier3: bool,
    page: u32,
    page_size: u32,
) -> StdResult<Binary> {
    let mut tier1_rounds: Option<Vec<RoundStruct>> = None;
    let mut tier2_rounds: Option<Vec<RoundStruct>> = None;
    let mut tier3_rounds: Option<Vec<RoundStruct>> = None;

    if tier1 {
        let tier1_rounds_storage = ReadonlyPrefixedStorage::multilevel(&[ROUNDS_STATE, &"tier1".to_string().as_bytes()], &deps.storage);
        let tier1_rounds_store = if let Some(result) = AppendStore::<RoundStruct, _>::attach(&tier1_rounds_storage) {
            result?
        } else {
            return to_binary(&QueryAnswer::GetRounds {
                tier1_rounds,
                tier2_rounds,
                tier3_rounds
            })
        };

        let rounds_iter = tier1_rounds_store
        .iter()
        .rev()
        .skip((page * page_size) as _)
        .take(page_size as _);

        let rounds: StdResult<Vec<RoundStruct>> = rounds_iter.collect();
        tier1_rounds = Some(rounds.unwrap());
    }

    if tier2 {
        let tier2_rounds_storage = ReadonlyPrefixedStorage::multilevel(&[ROUNDS_STATE, &"tier2".to_string().as_bytes()], &deps.storage);
        let tier2_rounds_store = if let Some(result) = AppendStore::<RoundStruct, _>::attach(&tier2_rounds_storage) {
            result?
        } else {
            return to_binary(&QueryAnswer::GetRounds {
                tier1_rounds,
                tier2_rounds,
                tier3_rounds
            })
        };

        let rounds_iter = tier2_rounds_store
        .iter()
        .rev()
        .skip((page * page_size) as _)
        .take(page_size as _);

        let rounds: StdResult<Vec<RoundStruct>> = rounds_iter.collect();
        tier2_rounds = Some(rounds.unwrap());
    }

    if tier3 {
        let tier3_rounds_storage = ReadonlyPrefixedStorage::multilevel(&[ROUNDS_STATE, &"tier3".to_string().as_bytes()], &deps.storage);
        let tier3_rounds_store = if let Some(result) = AppendStore::<RoundStruct, _>::attach(&tier3_rounds_storage) {
            result?
        } else {
            return to_binary(&QueryAnswer::GetRounds {
                tier1_rounds,
                tier2_rounds,
                tier3_rounds
            })
        };

        let rounds_iter = tier3_rounds_store
        .iter()
        .rev()
        .skip((page * page_size) as _)
        .take(page_size as _);

        let rounds: StdResult<Vec<RoundStruct>> = rounds_iter.collect();
        tier3_rounds = Some(rounds.unwrap());
    }
    
    to_binary(&QueryAnswer::GetRounds {
        tier1_rounds,
        tier2_rounds,
        tier3_rounds
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
