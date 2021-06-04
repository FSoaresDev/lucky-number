use std::{collections::HashMap, hash::Hash, path::Prefix};

use cosmwasm_std::{Api, Binary, CanonicalAddr, CosmosMsg, Env, Extern, HandleResponse, HumanAddr, InitResponse, LogAttribute, Querier, QueryResult, StdError, StdResult, Storage, Uint128, WasmMsg, from_binary, to_binary};
use cosmwasm_storage::{PrefixedStorage, ReadonlyPrefixedStorage};
use rand::Rng;
use rand_chacha::ChaChaRng;
use secret_toolkit::storage::{AppendStore, AppendStoreMut, TypedStore};
use sha2::{Digest, Sha256};
use rand_core::SeedableRng;
use crate::{msg::{CountResponse, HandleMsg, InitMsg, QueryAnswer, QueryMsg, Snip20Msg}, rand::sha_256, state::{RoundStruct, UserBetStruct, UserBetsStruct, load, may_load, save}};

/*
    5 min Lucky Number =>  1 sSCRT => 1 - 5
    1h Lucky Number =>  5 sSCRT => 1 - 15
    12h Lucky Number =>  10 sSCRT => 1-30
*/
pub const CONFIG_DATA: &[u8] = b"config";
pub const LUCKY_NUMBER_CONFIG_TIER_1: &[u8] = b"tier1";
pub const LUCKY_NUMBER_CONFIG_TIER_2: &[u8] = b"tier2";
pub const LUCKY_NUMBER_CONFIG_TIER_3: &[u8] = b"tier3";
pub const ROUNDS_STATE: &[u8] = b"rounds";
pub const BETS: &[u8] = b"bets";
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
        round_end_timestamp: None,
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
    new_round.users_picked_numbers_count = vec![0; *(&msg.tier1_max_rand_number) as usize];
    tier1_rounds_store.push(&new_round)?;

    let mut tier2_state = PrefixedStorage::new(LUCKY_NUMBER_CONFIG_TIER_2, &mut deps.storage);
    save(&mut tier2_state, b"entry_fee", &msg.tier2_entry_fee)?;
    save(&mut tier2_state, b"triggerer_fee", &msg.tier2_triggerer_fee)?;
    save(&mut tier2_state, b"min_entries", &msg.tier2_min_entries)?;
    save(&mut tier2_state, b"max_rand_number", &msg.tier2_max_rand_number)?;
    let mut tier2_rounds = PrefixedStorage::multilevel(&[ROUNDS_STATE, &"tier2".to_string().as_bytes()], &mut deps.storage);
    let mut tier2_rounds_store: AppendStoreMut<RoundStruct, _> = AppendStoreMut::attach_or_create(&mut tier2_rounds)?;
    new_round.users_picked_numbers_count = vec![0; *(&msg.tier2_max_rand_number) as usize];
    tier2_rounds_store.push(&new_round)?;

    let mut tier3_state = PrefixedStorage::new(LUCKY_NUMBER_CONFIG_TIER_3, &mut deps.storage);
    save(&mut tier3_state, b"entry_fee", &msg.tier3_entry_fee)?;
    save(&mut tier3_state, b"triggerer_fee", &msg.tier3_triggerer_fee)?;
    save(&mut tier3_state, b"min_entries", &msg.tier3_min_entries)?;
    save(&mut tier3_state, b"max_rand_number", &msg.tier3_max_rand_number)?;
    let mut tier3_rounds = PrefixedStorage::multilevel(&[ROUNDS_STATE, &"tier3".to_string().as_bytes()], &mut deps.storage);
    let mut tier3_rounds_store: AppendStoreMut<RoundStruct, _> = AppendStoreMut::attach_or_create(&mut tier3_rounds)?;
    new_round.users_picked_numbers_count = vec![0; *(&msg.tier3_max_rand_number) as usize];
    tier3_rounds_store.push(&new_round)?;

   let snip20_register_msg = to_binary(&Snip20Msg::register_receive(env.clone().contract_code_hash))?;

    let token_response: Option<CosmosMsg> = Some(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: msg.token_address,
        callback_code_hash: msg.token_hash,
        msg: snip20_register_msg,
        send: vec![],
    }));

    Ok(InitResponse {
        messages: vec![
           token_response.unwrap()
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

        _ => Err(StdError::generic_err("Handler not found!"))
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
    if msg != None { 
        let msg: HandleMsg = from_binary(&msg.unwrap())?; 
        if matches!(msg, HandleMsg::Receive { .. }) {
            return Err(StdError::generic_err(
                "Recursive call to receive() is not allowed",
            ));
        }

        if let HandleMsg::Bet {tier,number} = msg.clone() {
            return try_bet(deps, env.clone(), amount, from, number, tier)
        } else {
            return Err(StdError::generic_err(format!(
                "Receive handler not found!"
            )));
         }
    } else {
        return Err(StdError::generic_err(format!(
            "Receive handler not found!"
        )));
    }
}

pub fn try_bet<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    amount: Uint128,
    from: HumanAddr,
    number: i16,
    tier: i8
) -> StdResult<HandleResponse> {
    let user_address = deps.api.canonical_address(&from)?;

    // check if amount is correct for this tier
    let tier_config_key = 
        if tier == 1 { LUCKY_NUMBER_CONFIG_TIER_1 } 
        else if tier == 2 { LUCKY_NUMBER_CONFIG_TIER_2 } 
        else if tier == 3 { LUCKY_NUMBER_CONFIG_TIER_3 } 
        else { 
            return Err(StdError::generic_err(format!(
                "Tier invalid"
            )));
        };

    let tier_config = ReadonlyPrefixedStorage::new(tier_config_key, &deps.storage);
    let entry_fee_tier: Uint128 = load(&tier_config, b"entry_fee").unwrap();
    if entry_fee_tier != amount {
        return Err(StdError::generic_err(format!(
            "Amount invalid of tier choosen"
        )));
    }

    let tier_rounds_key: String = "tier".to_owned()  + &tier.to_string();
    let mut tier_rounds = PrefixedStorage::multilevel(&[ROUNDS_STATE, &tier_rounds_key.as_bytes()], &mut deps.storage);
    let mut tier_rounds_store: AppendStoreMut<RoundStruct, _> = AppendStoreMut::attach_or_create(&mut tier_rounds)?;

    let mut current_round_state = tier_rounds_store.get_at(tier_rounds_store.len() - 1).unwrap();

    // update round state
    current_round_state.pool_size = current_round_state.pool_size + amount;
    current_round_state.users_count = current_round_state.users_count + 1;
    current_round_state.users_picked_numbers_count[number as usize - 1] = current_round_state.users_picked_numbers_count[number as usize - 1] + 1;
    tier_rounds_store.set_at(tier_rounds_store.len()-1,&current_round_state);

    // how do i know if this user already bet on that tier/round 
    let mapping_key: String = "tier".to_owned() + &tier.to_string() + "_" + "round" + &current_round_state.round_number.to_string();
    let mut bets_storage = PrefixedStorage::new(BETS, &mut deps.storage);
    let mut user_bets: Option<UserBetsStruct> = may_load(&bets_storage, &user_address.as_slice())?;

    if user_bets.clone() != None && user_bets.clone().unwrap().bets.contains_key(&mapping_key) {
        return Err(StdError::generic_err(format!(
            "User already bet on this round / tier."
        )));
    }

    /*let mapping_key: String = "tier".to_owned() + &tier.to_string() + "_" + "round" + &current_round_state.round_number.to_string() + "_" + &user_address.to_string();
    let mut mappings = PrefixedStorage::new(MAPPING_BETS_TO_ROUNDS, &mut deps.storage);
    let map_result: Result<bool,StdError> = load(&mappings, mapping_key.as_bytes());
 
    if !map_result.is_err() && map_result.unwrap() == true {
        return Err(StdError::generic_err(format!(
            "User already bet on this round / tier."
        )));
    }
 
    save(&mut mappings, mapping_key.as_bytes(), &true)?;
*/

    //add user bet
    
    let user_bet: UserBetStruct = UserBetStruct {
        round_number: 0,
        tier,
        number,
        claimed_reward: false,
        timestamp: env.block.time
    };

    // { <user_address>: { "bet_keys": [...], "bets": {...} } }
    if user_bets.clone() == None {
        let mut hashmap: HashMap<String, UserBetStruct> = HashMap::new();
        hashmap.insert(mapping_key.clone(), user_bet);

        user_bets = Some(UserBetsStruct {
            bet_keys: vec![ mapping_key.clone() ],
            bets: hashmap
        })
    } else {
        user_bets.clone().unwrap().bet_keys.push(mapping_key.clone());
        user_bets.clone().unwrap().bets.insert(mapping_key.clone(), user_bet);
    }
    
    save(&mut bets_storage, &user_address.as_slice(), &user_bets)?;

    //let mut bets_storage = PrefixedStorage::multilevel(&[BETS, &user_address.as_slice()], &mut deps.storage);
    //let mut bets_store = AppendStoreMut::attach_or_create(&mut bets_storage)?;
    //bets_store.push(&user_bet)?;

    // add the bet number to the additional entropy array
    // As on ChaChaRng only up to 8 words are used, and 2 of them are the base entropy and the entropy sent by the trigger we will save only 6 users entropy on this array
    let mut config_data = PrefixedStorage::new(CONFIG_DATA, &mut deps.storage);
    let mut addition_entropy: Vec<_> = load(&config_data, b"addition_entropy").unwrap();
    if addition_entropy.len() >= 6 {
        addition_entropy[0] = number as u64;
        addition_entropy.rotate_right(1);
    } else {
        addition_entropy.push(number as u64)
    }
    save(&mut config_data, b"addition_entropy", &addition_entropy)?;

    return Ok(HandleResponse {
        messages: vec![],
        log: vec![],
        data: None,
    })
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
    round: u32
) -> StdResult<HandleResponse> {
    let user_address = deps.api.canonical_address(&env.message.sender)?;

    // Check if user bet on this tier/round
    let mapping_key: String = "tier".to_owned() + &tier.to_string() + "_" + "round" + &round.to_string();
    let mut bets_storage = PrefixedStorage::new(BETS, &mut deps.storage);
    let user_bets: Option<UserBetsStruct> = may_load(&bets_storage, &user_address.as_slice())?;

    if user_bets.clone() == None || !user_bets.clone().unwrap().bets.contains_key(&mapping_key) {
        return Err(StdError::generic_err(format!(
            "User does not have any redeemable bet on this tier/round!"
        )));
    }

    // check the user bet state
    let this_user_bets = user_bets.unwrap().bets;
    let this_user_bet = this_user_bets.get(&mapping_key).unwrap();

    if this_user_bet.claimed_reward {
        return Err(StdError::generic_err(format!(
            "This user already claimed the reward for this tier/round!"
        )));
    }

    // get that tier/round state
    let tier_rounds_key: String = "tier".to_owned()  + &tier.to_string();
    let mut tier_rounds = PrefixedStorage::multilevel(&[ROUNDS_STATE, &tier_rounds_key.as_bytes()], &mut deps.storage);
    let mut tier_rounds_store: AppendStoreMut<RoundStruct, _> = AppendStoreMut::attach_or_create(&mut tier_rounds)?;
    let mut round_state = tier_rounds_store.get_at(round).unwrap();

    // check if round is finished with the lucky number field
    if round_state.lucky_number == None {
        // if it is not finished, the user wants to withdrawl his bet!

    } else {
        // the round is finished so the user wants to redeem the reward

        // check if user is not winner
        if round_state.lucky_number.unwrap() != this_user_bet.number {
            return Err(StdError::generic_err(format!(
                "User is not a winner! The bet number is not equal to the lucky number for this tier/round!"
            )));
        }

        // winner logic!


    }

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
    // TODO: check if it is the triggerer
    
    let config_data = ReadonlyPrefixedStorage::new(CONFIG_DATA, &deps.storage);
    // Generate seed vector: original entropy + this request entropy + max 6 entropy stored from users
    let base_entropy = load(&config_data, b"base_entropy").unwrap();
    let mut addition_entropy: Vec<_> = load(&config_data, b"addition_entropy").unwrap();

    addition_entropy.push(base_entropy);
    addition_entropy.push(entropy.clone().to_be_bytes());
    
    let mut hasher = Sha256::new();
    addition_entropy.iter().for_each(|el| hasher.update(el));
    let seed:[u8; 32] = hasher.finalize().into();
    let mut rng = ChaChaRng::from_seed(seed); // ChaChaRng::from_seed Only up to 8 words are used;

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
            updated_round.round_end_timestamp = Some(env.block.time);
            tier1_rounds_store.set_at(tier1_rounds_store.len()-1,&updated_round);

            //new round
            let new_round: RoundStruct = RoundStruct {
                round_number: tier1_rounds_store.len(),
                lucky_number: None,
                users_count: 0,
                round_end_timestamp: None,
                pool_size: Uint128(0),
                users_picked_numbers_count: vec![0; (max_rand_number_tier1) as usize]
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
            updated_round.round_end_timestamp = Some(env.block.time);
            tier2_rounds_store.set_at(tier2_rounds_store.len()-1,&updated_round);

            //new round
            let new_round: RoundStruct = RoundStruct {
                round_number: tier2_rounds_store.len(),
                lucky_number: None,
                users_count: 0,
                round_end_timestamp: None,
                pool_size: Uint128(0),
                users_picked_numbers_count: vec![0; (max_rand_number_tier2) as usize]
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
            updated_round.round_end_timestamp = Some(env.block.time);
            tier3_rounds_store.set_at(tier3_rounds_store.len()-1,&updated_round);

            //new round
            let new_round: RoundStruct = RoundStruct {
                round_number: tier3_rounds_store.len(),
                lucky_number: None,
                users_count: 0,
                round_end_timestamp: None,
                pool_size: Uint128(0),
                users_picked_numbers_count: vec![0; (max_rand_number_tier3) as usize]
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
        QueryMsg::GetUserBets { user_address} => to_binary(&query_user_bets(deps, user_address)?),
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

fn query_user_bets<S: Storage, A: Api, Q: Querier>(deps: &Extern<S, A, Q>, user_address: HumanAddr) -> QueryResult  {
    let user_address_canonical = &deps.api.canonical_address(&user_address)?;

    let bets_storage = ReadonlyPrefixedStorage::new(BETS, &deps.storage);
    let user_bets: Option<UserBetsStruct> = load(&bets_storage, &user_address_canonical.as_slice())?;

    let mut user_bets_vec = vec![];

    if user_bets != None {
        for key in user_bets.clone().unwrap().bet_keys.iter() {
            user_bets_vec.push(user_bets.clone().unwrap().bets[key].clone())
        }

        to_binary(&QueryAnswer::GetUserBets {
            user_bet_keys: user_bets.unwrap().bet_keys,
            user_bets: user_bets_vec
        })
    } else {
        to_binary(&QueryAnswer::GetUserBets {
            user_bet_keys: vec![],
            user_bets: user_bets_vec
        })
    }
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