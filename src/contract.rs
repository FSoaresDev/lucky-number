use std::{collections::HashMap, hash::Hash, path::Prefix};

use cosmwasm_std::{Api, Binary, CanonicalAddr, CosmosMsg, Empty, Env, Extern, HandleResponse, HandleResult, HumanAddr, InitResponse, LogAttribute, Querier, QueryResult, ReadonlyStorage, StdError, StdResult, Storage, Uint128, WasmMsg, from_binary, to_binary};
use cosmwasm_storage::{PrefixedStorage, ReadonlyPrefixedStorage};
use rand::Rng;
use rand_chacha::ChaChaRng;
use secret_toolkit::{snip20::{self, transfer_msg}, storage::{AppendStore, AppendStoreMut, TypedStore}};
use sha2::{Digest, Sha256};
use rand_core::SeedableRng;
use crate::{msg::{CountResponse, HandleAnswer, HandleMsg, InitMsg, QueryAnswer, QueryMsg, ResponseStatus, Snip20Msg, TierConfig}, rand::sha_256, state::{RoundStruct, UserBetStruct, UserBetsStruct, load, may_load, save}, viewing_key::{VIEWING_KEY_SIZE, ViewingKey}};

/*
    5 min Lucky Number =>  1 sSCRT => 1 - 5
    1h Lucky Number =>  5 sSCRT => 1 - 15
    12h Lucky Number =>  10 sSCRT => 1-30
*/
pub const CONFIG_DATA: &[u8] = b"config";
pub const PREFIX_VIEW_KEY: &[u8] = b"viewingkey";
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
        round_end_pool_size: None,
        pool_size: Uint128(0),
        users_picked_numbers_count: vec![0; 0],
        winner_users_count: None
    };

    let mut config_data = PrefixedStorage::new(CONFIG_DATA, &mut deps.storage);
    save(&mut config_data, b"owner", &deps.api.canonical_address(&env.message.sender)?)?;
    save(&mut config_data, b"triggerer", &msg.triggerer_address)?;
    save(&mut config_data, b"token_address", &msg.token_address)?;
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

    Ok(InitResponse {
        messages: vec![
           snip20::register_receive_msg(
            env.contract_code_hash.clone(),
            None,
            1,
            msg.token_hash.clone(),
            msg.token_address.clone(),
        )?,
        snip20::set_viewing_key_msg(
            msg.token_vk.clone(),
            None,
            BLOCK_SIZE, // This is private data, need to pad
            msg.token_hash.clone(),
            msg.token_address.clone(),
        )?,
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
        // Users
        HandleMsg::CreateViewingKey { entropy, .. } => try_create_key(deps, env, &entropy),
        HandleMsg::SetViewingKey { key, .. } => try_set_key(deps, env, &key),

        // Bet
        HandleMsg::Receive { sender, from, amount, msg } => try_receive(deps, env, sender, from, amount, msg),
        HandleMsg::Withdrawl { tier, round } => try_withdrawl(deps, env, tier, round),

        // Triggerer
        HandleMsg::TriggerLuckyNumber { tier1, tier2, tier3, entropy } => try_trigger_lucky_number(deps, env, tier1, tier2, tier3, entropy),
        
        // Admin
        HandleMsg::ChangeAdmin { admin } => try_change_admin(deps, env, admin),
        HandleMsg::ChangeTriggerer { triggerer } => try_change_triggerer(deps, env, triggerer),
        HandleMsg::ChangeTier { tier, entry_fee, triggerer_fee, min_entries, max_rand_number } => try_change_tier(deps, env, tier, entry_fee, triggerer_fee, min_entries, max_rand_number),

        _ => Err(StdError::generic_err("Handler not found!"))
    }
}

fn try_create_key<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    entropy: &str,
) -> HandleResult {
    // create and store the key
    let config_data = ReadonlyPrefixedStorage::new(CONFIG_DATA, &deps.storage);
    let entropy_base: Vec<u8> = load(&config_data, b"entropy")?;
    let key = ViewingKey::new(&env, &entropy_base, entropy.as_ref());
    let message_sender = &deps.api.canonical_address(&env.message.sender)?;
    let mut key_store = PrefixedStorage::new(PREFIX_VIEW_KEY, &mut deps.storage);
    save(&mut key_store, message_sender.as_slice(), &key.to_hashed())?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![],
        data: Some(to_binary(&HandleAnswer::ViewingKey {
            key: format!("{}", key),
        })?),
    })
}

fn try_set_key<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    key: &String,
) -> HandleResult {
    let vk = ViewingKey(key.clone());
    let message_sender = &deps.api.canonical_address(&env.message.sender)?;

    let mut key_store = PrefixedStorage::new(PREFIX_VIEW_KEY, &mut deps.storage);
    save(&mut key_store, message_sender.as_slice(), &vk.to_hashed())?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![],
        data: Some(to_binary(&HandleAnswer::Status {
            status: ResponseStatus::Success,
            message: None
        })?),
    })
}

pub fn try_receive<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    _sender: HumanAddr,
    from: HumanAddr,
    amount: Uint128,
    msg: Binary,
) -> StdResult<HandleResponse> {
        let msg: HandleMsg = from_binary(&msg)?; 

        if let HandleMsg::Bet {tier,number} = msg.clone() {
            let config_data = ReadonlyPrefixedStorage::new(CONFIG_DATA, &mut deps.storage);
            let token_address: HumanAddr = load(&config_data, b"token_address")?;
            if env.message.sender != token_address {
                return Err(StdError::generic_err(format!(
                    "Invalid token sent!"
                )));
            } else {
                return try_bet(deps, env.clone(), amount, from, number, tier)
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

    // check correct entry fee for the tier selected    
    let tier_config = ReadonlyPrefixedStorage::new(tier_config_key, &deps.storage);
    let entry_fee_tier: Uint128 = load(&tier_config, b"entry_fee")?;
    if entry_fee_tier != amount {
        return Err(StdError::generic_err(format!(
            "Amount invalid of tier choosen"
        )));
    }

    // check if number is inside the range for that tier
    let max_rand_number_tier: i16 = load(&tier_config, b"max_rand_number")?;
    if number < 1 || number > max_rand_number_tier {
        return Err(StdError::generic_err(format!(
            "Number outside valid range for this tier!"
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
    let user_bets: Result<Option<UserBetsStruct>,StdError> = load(&bets_storage, &user_address.as_slice());
    
    //add user bet
    let user_bet: UserBetStruct = UserBetStruct {
        round_number: current_round_state.round_number,
        tier,
        number,
        claimed_reward: false,
        timestamp: env.block.time
    };
    let mut user_bets_modified;
    
    // { <user_address>: { "bet_keys": [...], "bets": {...} } }
    if user_bets.is_err() {
        let mut hashmap: HashMap<String, UserBetStruct> = HashMap::new();
        hashmap.insert(mapping_key.clone(), user_bet);

        user_bets_modified = UserBetsStruct {
            bet_keys: vec![ mapping_key.clone() ],
            bets: hashmap
        }
    } else {
        let user_bets_unwraped = user_bets.unwrap().unwrap();
        if user_bets_unwraped.bets.contains_key(&mapping_key) {
            return Err(StdError::generic_err(format!(
                "User already bet on this round / tier."
            )));
       }
       user_bets_modified = user_bets_unwraped;
       user_bets_modified.bet_keys.push(mapping_key.clone());
       user_bets_modified.bets.insert(mapping_key.clone(), user_bet);
    }

    save(&mut bets_storage, &user_address.as_slice(), &Some(user_bets_modified))?;

    // add the bet number to the additional entropy array
    // As on ChaChaRng only up to 8 words are used, and 2 of them are the base entropy and the entropy sent by the trigger we will save only 6 users entropy on this array
    let mut config_data = PrefixedStorage::new(CONFIG_DATA, &mut deps.storage);
    let mut addition_entropy: Vec<_> = load(&config_data, b"addition_entropy")?;
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
        data: Some(to_binary(&HandleAnswer::Status {
            status: ResponseStatus::Success,
            message: None
        })?),
    })
}

pub fn try_change_admin<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    admin: HumanAddr,
) -> StdResult<HandleResponse> {
    let sender = deps.api.canonical_address(&env.message.sender)?;
    let mut config_data = PrefixedStorage::new(CONFIG_DATA, &mut deps.storage);
    let owner_address: CanonicalAddr = load(&config_data, b"owner")?;

    if sender == owner_address {
        save(&mut config_data, b"owner", &deps.api.canonical_address(&admin)?)?;
        return Ok(HandleResponse {
            messages: vec![],
            log: vec![],
            data: Some(to_binary(&HandleAnswer::Status {
                status: ResponseStatus::Success,
                message: None
            })?),
        })
    } else {
        return Err(StdError::generic_err(format!(
            "User does not permissions to change owner!"
        )));
    }
}

pub fn try_change_triggerer<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    triggerer: HumanAddr,
) -> StdResult<HandleResponse> {
    let sender = deps.api.canonical_address(&env.message.sender)?;
    let mut config_data = PrefixedStorage::new(CONFIG_DATA, &mut deps.storage);
    let owner_address: CanonicalAddr = load(&config_data, b"owner")?;

    if sender == owner_address {
        save(&mut config_data, b"triggerer", &triggerer)?;
        return Ok(HandleResponse {
            messages: vec![],
            log: vec![],
            data: Some(to_binary(&HandleAnswer::Status {
                status: ResponseStatus::Success,
                message: None
            })?),
        })
    } else {
        return Err(StdError::generic_err(format!(
            "User does not permissions to change triggerer!"
        )));
    }
    
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
    let sender = deps.api.canonical_address(&env.message.sender)?;
    let config_data = ReadonlyPrefixedStorage::new(CONFIG_DATA, &deps.storage);
    let owner_address: CanonicalAddr = load(&config_data, b"owner")?;

    if sender == owner_address {
        let tier_config_key = 
            if tier == 1 { LUCKY_NUMBER_CONFIG_TIER_1 } 
            else if tier == 2 { LUCKY_NUMBER_CONFIG_TIER_2 } 
            else if tier == 3 { LUCKY_NUMBER_CONFIG_TIER_3 } 
            else { 
                        return Err(StdError::generic_err(format!(
                            "Tier invalid"
                        )));
            };
            
        let mut tier_state = PrefixedStorage::new(tier_config_key, &mut deps.storage);
        save(&mut tier_state, b"entry_fee", &entry_fee)?;
        save(&mut tier_state, b"triggerer_fee", &triggerer_fee)?;
        save(&mut tier_state, b"min_entries", &min_entries)?;
        save(&mut tier_state, b"max_rand_number", &max_rand_number)?;

        return Ok(HandleResponse {
            messages: vec![],
            log: vec![],
            data: Some(to_binary(&HandleAnswer::Status {
                status: ResponseStatus::Success,
                message: None
            })?),
        })
    } else {
        return Err(StdError::generic_err(format!(
            "User does not permissions to change tiers!"
        )));
    }
}

pub fn try_withdrawl<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    tier: i8,
    round: u32
) -> StdResult<HandleResponse> {
    let user_address = deps.api.canonical_address(&env.message.sender)?;

    // check the tier entry amount
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
    let entry_fee_tier: Uint128 = load(&tier_config, b"entry_fee")?;

    //get transfer token info
    let config_data = ReadonlyPrefixedStorage::new(CONFIG_DATA, &deps.storage);
    let token_address: HumanAddr = load(&config_data, b"token_address")?;
    let token_hash: String  = load(&config_data, b"token_hash")?;

    // get that tier/round state
    let tier_rounds_key: String = "tier".to_owned()  + &tier.to_string();
    let tier_rounds = ReadonlyPrefixedStorage::multilevel(&[ROUNDS_STATE, &tier_rounds_key.as_bytes()], &deps.storage);
    let tier_rounds_store: AppendStore<RoundStruct, _> = AppendStore::attach(&tier_rounds).unwrap().unwrap();
    let round_state = tier_rounds_store.get_at(round).unwrap();
     
    // Check if user bet on this tier/round
    let mapping_key: String = "tier".to_owned() + &tier.to_string() + "_" + "round" + &round.to_string();
    let mut bets_storage = ReadonlyPrefixedStorage::new(BETS, &deps.storage);
    let user_bets: Option<UserBetsStruct> = load(&bets_storage, &user_address.as_slice())?;

    if user_bets.clone() == None || !user_bets.clone().unwrap().bets.contains_key(&mapping_key) {
        return Err(StdError::generic_err(format!(
            "User does not have any redeemable bet on this tier/round!"
        )));
    }

    // check the user bet state
    let mut this_user_bets = user_bets.unwrap();

    if this_user_bets.bets.get(&mapping_key).unwrap().claimed_reward {
        return Err(StdError::generic_err(format!(
            "This user already claimed the reward for this tier/round!"
        )));
    }

    let mut transfer_result: CosmosMsg = CosmosMsg::Custom(Empty {});

    // check if round is finished with the lucky number field
    if round_state.lucky_number == None {
        // if the round is not finished, the user wants to withdrawl his bet!

        // transfer the tokens
        transfer_result = transfer_msg(
            env.message.sender.clone(),
            entry_fee_tier,
            None,
            BLOCK_SIZE,
            token_hash,
            token_address
        ).unwrap();

        // clear round state
        let mut tier_rounds = PrefixedStorage::multilevel(&[ROUNDS_STATE, &tier_rounds_key.as_bytes()], &mut deps.storage);
        let mut tier_rounds_store: AppendStoreMut<RoundStruct, _> = AppendStoreMut::attach_or_create(&mut tier_rounds)?;
        let mut round_state = tier_rounds_store.get_at(round).unwrap();

        round_state.users_count = round_state.users_count - 1;
        round_state.pool_size = (round_state.pool_size - entry_fee_tier)?;
        round_state.users_picked_numbers_count[this_user_bets.bets.get(&mapping_key).unwrap().number as usize - 1] = round_state.users_picked_numbers_count[this_user_bets.bets.get(&mapping_key).unwrap().number as usize - 1] - 1;
   
        tier_rounds_store.set_at(round_state.round_number,&round_state);

        // clear user bets
        let mut bets_storage = PrefixedStorage::new(BETS, &mut deps.storage);
        if let Some(index) = this_user_bets.bet_keys.iter().position(|value| value == &mapping_key) {
            this_user_bets.bet_keys.remove(index);
        } 
        this_user_bets.bets.remove(&mapping_key.clone());
        save(&mut bets_storage, &user_address.as_slice(), &Some(this_user_bets))?;
    } else {
        // the round is finished so the user wants to redeem the reward

        // check if user is not winner
        if round_state.lucky_number.unwrap() != this_user_bets.bets.get(&mapping_key).unwrap().number {
            return Err(StdError::generic_err(format!(
                "User is not a winner! The bet number is not equal to the lucky number for this tier/round!"
            )));
        }

        // winner logic!
        let win_players_count: u128 = *(round_state.users_picked_numbers_count.get((round_state.lucky_number.unwrap() - 1) as usize)).unwrap() as u128;
        let amount_for_this_winner = round_state.pool_size.multiply_ratio(Uint128(1), Uint128(win_players_count));

        transfer_result = transfer_msg(
            env.message.sender.clone(),
            amount_for_this_winner,
            None,
            BLOCK_SIZE,
            token_hash,
            token_address
        ).unwrap();       

        //
        // update user bets
        let mut bets_storage = PrefixedStorage::new(BETS, &mut deps.storage);
        let current_bet_state =  this_user_bets.bets.get(&mapping_key);
        if current_bet_state != None {
            let mut new_bet_state: UserBetStruct = current_bet_state.unwrap().clone();
            new_bet_state.claimed_reward = true;
            this_user_bets.bets.insert(mapping_key, new_bet_state.to_owned());
        } else {
            return Err(StdError::generic_err(format!(
                "Cannot Update User Bet state!"
            )));
        }

        save(&mut bets_storage, &user_address.as_slice(), &Some(this_user_bets))?;
    }

    Ok(HandleResponse {
        messages: vec![
            transfer_result
        ],
        log: vec![],
        data: Some(to_binary(&HandleAnswer::Status {
            status: ResponseStatus::Success,
            message: None,
        })?),
    })
}

pub fn try_trigger_lucky_number<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    tier1: bool, 
    tier2: bool,  
    tier3: bool, 
    entropy: u64
) -> StdResult<HandleResponse> {
    let config_data = ReadonlyPrefixedStorage::new(CONFIG_DATA, &deps.storage);

    let triggerer_address: HumanAddr = load(&config_data, b"triggerer")?;
    if triggerer_address != env.message.sender {
        return Err(StdError::generic_err(format!(
            "Not the valid triggerer!"
        )));
    }
    let token_address: HumanAddr = load(&config_data, b"token_address")?;
    let token_hash: String  = load(&config_data, b"token_hash")?;

    // Generate seed vector: original entropy + this request entropy + max 6 entropy stored from users
    let base_entropy = load(&config_data, b"base_entropy")?;
    let mut addition_entropy: Vec<_> = load(&config_data, b"addition_entropy")?;

    addition_entropy.push(base_entropy);
    addition_entropy.push(entropy.clone().to_be_bytes());
    
    let mut hasher = Sha256::new();
    addition_entropy.iter().for_each(|el| hasher.update(el));
    let seed:[u8; 32] = hasher.finalize().into();
    let mut rng = ChaChaRng::from_seed(seed); // ChaChaRng::from_seed Only up to 8 words are used;

    let mut lucky_number_tier_1: i16 = 0;
    let mut lucky_number_tier_2: i16 = 0;
    let mut lucky_number_tier_3: i16 = 0;
    let mut transfer_result: CosmosMsg = CosmosMsg::Custom(Empty {});

    if tier1 == true {
        let tier1_config = ReadonlyPrefixedStorage::new(LUCKY_NUMBER_CONFIG_TIER_1, &deps.storage);
        let min_entries_tier1: i16 = load(&tier1_config, b"min_entries")?;
        let entry_fee_tier1: Uint128 = load(&tier1_config, b"entry_fee")?;
        let max_rand_number_tier1: i16 = load(&tier1_config, b"max_rand_number")?;
        let triggerer_fee_tier1: Uint128 = load(&tier1_config, b"triggerer_fee")?;

        let mut tier1_rounds = PrefixedStorage::multilevel(&[ROUNDS_STATE, &"tier1".to_string().as_bytes()], &mut deps.storage);
        let mut tier1_rounds_store: AppendStoreMut<RoundStruct, _> = AppendStoreMut::attach_or_create(&mut tier1_rounds)?;
        let tier1_cur_round: RoundStruct = tier1_rounds_store.get_at(tier1_rounds_store.len() - 1).unwrap();
        
        let pool_size_cur_round_tier1: Uint128 =  tier1_cur_round.pool_size;

        // check if there are enougth pool size (pool_size >= min_entries * entry_fee)
        if pool_size_cur_round_tier1 >= Uint128(min_entries_tier1 as u128).multiply_ratio(entry_fee_tier1, Uint128(1)) {
            lucky_number_tier_1 = rng.gen_range(1,max_rand_number_tier1 + 1);

            //update round
            let mut updated_round = tier1_cur_round;
            updated_round.lucky_number = Some(lucky_number_tier_1);
            updated_round.round_end_timestamp = Some(env.block.time);
            updated_round.pool_size = (updated_round.pool_size - triggerer_fee_tier1)?;
            updated_round.round_end_pool_size = Some(updated_round.pool_size);
            let mut next_round_pool_size = Uint128(0);
            // Check if any winner, if not the pool size will transfer to the next round so this round state will be 0!
            let win_players_count: u128 = *(updated_round.users_picked_numbers_count.get((lucky_number_tier_1 - 1) as usize)).unwrap() as u128;
            updated_round.winner_users_count = Some(win_players_count as u32);
            if win_players_count == 0 {
                next_round_pool_size = updated_round.pool_size;
                updated_round.pool_size = Uint128(0);
            }
            tier1_rounds_store.set_at(tier1_rounds_store.len()-1,&updated_round);

            //send trigger fee to triggerer
            transfer_result = transfer_msg(
                triggerer_address.clone(),
                triggerer_fee_tier1,
                None,
                BLOCK_SIZE,
                token_hash.clone(),
                token_address.clone()
            ).unwrap();

            //new round
            let new_round: RoundStruct = RoundStruct {
                round_number: tier1_rounds_store.len(),
                lucky_number: None,
                users_count: 0,
                round_end_timestamp: None,
                round_end_pool_size: None,
                pool_size: next_round_pool_size,
                users_picked_numbers_count: vec![0; (max_rand_number_tier1) as usize],
                winner_users_count: None
            };
            tier1_rounds_store.push(&new_round)?;
        }
    }

    if tier2 == true {
        let tier2_config = ReadonlyPrefixedStorage::new(LUCKY_NUMBER_CONFIG_TIER_2, &deps.storage);
        let min_entries_tier2: i16 = load(&tier2_config, b"min_entries")?;
        let entry_fee_tier2: Uint128 = load(&tier2_config, b"entry_fee")?;
        let max_rand_number_tier2: i16 = load(&tier2_config, b"max_rand_number")?;
        let triggerer_fee_tier2: Uint128 = load(&tier2_config, b"triggerer_fee")?;

        let mut tier2_rounds = PrefixedStorage::multilevel(&[ROUNDS_STATE, &"tier2".to_string().as_bytes()], &mut deps.storage);
        let mut tier2_rounds_store: AppendStoreMut<RoundStruct, _> = AppendStoreMut::attach_or_create(&mut tier2_rounds)?;
        let tier2_cur_round: RoundStruct = tier2_rounds_store.get_at(tier2_rounds_store.len() - 1).unwrap();
        
        let pool_size_cur_round_tier2: Uint128 =  tier2_cur_round.pool_size;

        // check if there are enougth pool size (pool_size >= min_entries * entry_fee)
        if pool_size_cur_round_tier2 >= Uint128(min_entries_tier2 as u128).multiply_ratio(entry_fee_tier2, Uint128(1)) {
            lucky_number_tier_2 = rng.gen_range(1,max_rand_number_tier2 + 1);

            //update round
            let mut updated_round = tier2_cur_round;
            updated_round.lucky_number = Some(lucky_number_tier_2);
            updated_round.round_end_timestamp = Some(env.block.time);
            updated_round.pool_size = (updated_round.pool_size - triggerer_fee_tier2)?;
            updated_round.round_end_pool_size = Some(updated_round.pool_size);
            let mut next_round_pool_size = Uint128(0);
            // Check if any winner, if not the pool size will transfer to the next round so this round state will be 0!
            let win_players_count: u128 = *(updated_round.users_picked_numbers_count.get((lucky_number_tier_2 - 1) as usize)).unwrap() as u128;
            updated_round.winner_users_count = Some(win_players_count as u32);
            if win_players_count == 0 {
                next_round_pool_size = updated_round.pool_size;
                updated_round.pool_size = Uint128(0);
            }
            tier2_rounds_store.set_at(tier2_rounds_store.len()-1,&updated_round);

            //send trigger fee to triggerer
            transfer_result = transfer_msg(
                triggerer_address.clone(),
                triggerer_fee_tier2,
                None,
                BLOCK_SIZE,
                token_hash.clone(),
                token_address.clone()
            ).unwrap();

            //new round
            let new_round: RoundStruct = RoundStruct {
                round_number: tier2_rounds_store.len(),
                lucky_number: None,
                users_count: 0,
                round_end_timestamp: None,
                round_end_pool_size: None,
                pool_size: next_round_pool_size,
                users_picked_numbers_count: vec![0; (max_rand_number_tier2) as usize],
                winner_users_count: None
            };
            tier2_rounds_store.push(&new_round)?;
        }
    }

    if tier3 == true {
        let tier3_config = ReadonlyPrefixedStorage::new(LUCKY_NUMBER_CONFIG_TIER_3, &deps.storage);
        let min_entries_tier3: i16 = load(&tier3_config, b"min_entries")?;
        let entry_fee_tier3: Uint128 = load(&tier3_config, b"entry_fee")?;
        let max_rand_number_tier3: i16 = load(&tier3_config, b"max_rand_number")?;
        let triggerer_fee_tier3: Uint128 = load(&tier3_config, b"triggerer_fee")?;

        let mut tier3_rounds = PrefixedStorage::multilevel(&[ROUNDS_STATE, &"tier3".to_string().as_bytes()], &mut deps.storage);
        let mut tier3_rounds_store: AppendStoreMut<RoundStruct, _> = AppendStoreMut::attach_or_create(&mut tier3_rounds)?;
        let tier3_cur_round: RoundStruct = tier3_rounds_store.get_at(tier3_rounds_store.len() - 1).unwrap();
        
        let pool_size_cur_round_tier3: Uint128 =  tier3_cur_round.pool_size;

        // check if there are enougth pool size (pool_size >= min_entries * entry_fee)
        if pool_size_cur_round_tier3 >= Uint128(min_entries_tier3 as u128).multiply_ratio(entry_fee_tier3, Uint128(1)) {
            lucky_number_tier_3 = rng.gen_range(1,max_rand_number_tier3 + 1);

            //update round
            let mut updated_round = tier3_cur_round;
            updated_round.lucky_number = Some(lucky_number_tier_3);
            updated_round.round_end_timestamp = Some(env.block.time);
            updated_round.pool_size = (updated_round.pool_size - triggerer_fee_tier3)?;
            updated_round.round_end_pool_size = Some(updated_round.pool_size);
            let mut next_round_pool_size = Uint128(0);
            // Check if any winner, if not the pool size will transfer to the next round so this round state will be 0!
            let win_players_count: u128 = *(updated_round.users_picked_numbers_count.get((lucky_number_tier_3 - 1) as usize)).unwrap() as u128;
            updated_round.winner_users_count = Some(win_players_count as u32);
            if win_players_count == 0 {
                next_round_pool_size = updated_round.pool_size;
                updated_round.pool_size = Uint128(0);
            }
            tier3_rounds_store.set_at(tier3_rounds_store.len()-1,&updated_round);

            //send trigger fee to triggerer
            transfer_result = transfer_msg(
                triggerer_address.clone(),
                triggerer_fee_tier3,
                None,
                BLOCK_SIZE,
                token_hash.clone(),
                token_address.clone()
            ).unwrap();

            //new round
            let new_round: RoundStruct = RoundStruct {
                round_number: tier3_rounds_store.len(),
                lucky_number: None,
                users_count: 0,
                round_end_timestamp: None,
                round_end_pool_size: None,
                pool_size: next_round_pool_size,
                users_picked_numbers_count: vec![0; (max_rand_number_tier3) as usize],
                winner_users_count: None
            };
            tier3_rounds_store.push(&new_round)?;
        }
    }

    return Ok(HandleResponse {
        messages: vec![
            transfer_result
        ],
        log: vec![
            //LogAttribute {key: "lucky_number_tier_1".to_string(), value: lucky_number_tier_1.to_string()},
            //LogAttribute {key: "lucky_number_tier_2".to_string(), value: lucky_number_tier_2.to_string()},
            //LogAttribute {key: "lucky_number_tier_3".to_string(), value: lucky_number_tier_3.to_string()},
        ],
        data: Some(to_binary(&HandleAnswer::Status {
            status: ResponseStatus::Success,
            message: None,
        })?),
    });
} 

pub fn query<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetTriggerer {} => to_binary(&query_triggerer(deps)?),
        QueryMsg::GetUserBets { user_address, viewing_key, keys} => to_binary(&query_user_bets(deps, user_address, viewing_key, keys)?),
        QueryMsg::GetPaginatedUserBets { user_address, viewing_key, page, page_size} => to_binary(&query_paginated_user_bets(deps, user_address, viewing_key, page, page_size)?),
        QueryMsg::GetPaginatedRounds {tier1, tier2, tier3, page, page_size} => to_binary(&query_paginated_rounds(deps,tier1, tier2, tier3, page, page_size)?),
        QueryMsg::GetRounds {tier1_rounds, tier2_rounds, tier3_rounds} => to_binary(&query_rounds(deps,tier1_rounds, tier2_rounds, tier3_rounds)?),
        QueryMsg::GetTierConfigs {tier1, tier2, tier3} => to_binary(&query_tier_configs(deps,tier1, tier2, tier3)?),
        QueryMsg::CheckTriggers{} => to_binary(&query_check_triggers(deps)?),
    }
}

fn query_triggerer<S: Storage, A: Api, Q: Querier>(deps: &Extern<S, A, Q>) -> QueryResult  {
    let config_data = ReadonlyPrefixedStorage::new(CONFIG_DATA, &deps.storage);
    let triggerer_address: HumanAddr = load(&config_data, b"triggerer")?;

    to_binary(&QueryAnswer::GetTriggerer {
        triggerer: triggerer_address
    })
}

fn query_user_bets<S: Storage, A: Api, Q: Querier>(deps: &Extern<S, A, Q>, user_address: HumanAddr, viewing_key: String, keys: Vec<String>) -> QueryResult  {
    let mut user_bets: Vec<UserBetStruct> = vec![];
    
    let user_address_canonical = &deps.api.canonical_address(&user_address)?;

    if !is_key_valid(&deps.storage, user_address_canonical, viewing_key)? {
        return Err(StdError::generic_err(format!(
            "User+VK not valid!"
        )));
    }

    let bets_storage = ReadonlyPrefixedStorage::new(BETS, &deps.storage);
    let user_bets_store: Option<UserBetsStruct> = load(&bets_storage, &user_address_canonical.as_slice())?;

    if user_bets_store == None {
        return to_binary(&QueryAnswer::GetUserBets {
            user_bets
        })
    } else {
        let user_bets_store_unwrapped = user_bets_store.unwrap();
        for key in keys {
            let bet_state = user_bets_store_unwrapped.bets.get(&key);
            if bet_state != None {
                user_bets.push(bet_state.unwrap().to_owned());
            }
        }
    }

    return to_binary(&QueryAnswer::GetUserBets {
        user_bets
    })
}

fn query_paginated_user_bets<S: Storage, A: Api, Q: Querier>(deps: &Extern<S, A, Q>, 
    user_address: HumanAddr, 
    viewing_key: String,
    page: u32,
    page_size: u32
) -> QueryResult  {
    let mut user_bets : Vec<UserBetStruct> = vec![];
    let mut bet_rounds: Vec<RoundStruct> = vec![];
    let mut user_bets_total_count = 0;

    let user_address_canonical = &deps.api.canonical_address(&user_address)?;
    if !is_key_valid(&deps.storage, user_address_canonical, viewing_key)? {
        return Err(StdError::generic_err(format!(
            "User+VK not valid!"
        )));
    }

    let tier1_rounds_storage = ReadonlyPrefixedStorage::multilevel(&[ROUNDS_STATE, &"tier1".to_string().as_bytes()], &deps.storage);
    let tier1_rounds_store = if let Some(result) = AppendStore::<RoundStruct, _>::attach(&tier1_rounds_storage) {
        result?
    } else {
        return to_binary(&QueryAnswer::GetPaginatedUserBets {
            user_bets,
            bet_rounds,
            user_bets_total_count
        })
    };
    let tier2_rounds_storage = ReadonlyPrefixedStorage::multilevel(&[ROUNDS_STATE, &"tier2".to_string().as_bytes()], &deps.storage);
    let tier2_rounds_store = if let Some(result) = AppendStore::<RoundStruct, _>::attach(&tier2_rounds_storage) {
        result?
    } else {
        return to_binary(&QueryAnswer::GetPaginatedUserBets {
            user_bets,
            bet_rounds,
            user_bets_total_count
        })
    };
    let tier3_rounds_storage = ReadonlyPrefixedStorage::multilevel(&[ROUNDS_STATE, &"tier3".to_string().as_bytes()], &deps.storage);
    let tier3_rounds_store = if let Some(result) = AppendStore::<RoundStruct, _>::attach(&tier3_rounds_storage) {
        result?
    } else {
        return to_binary(&QueryAnswer::GetPaginatedUserBets {
            user_bets,
            bet_rounds,
            user_bets_total_count
        })
    };
    let bets_storage = ReadonlyPrefixedStorage::new(BETS, &deps.storage);
    let user_bets_store: Option<UserBetsStruct> = load(&bets_storage, &user_address_canonical.as_slice())?;

    if user_bets_store == None {
        return to_binary(&QueryAnswer::GetPaginatedUserBets {
            user_bets,
            bet_rounds,
            user_bets_total_count
        })
    } else {
        let user_bets_store_unwrapped = user_bets_store.unwrap();
        user_bets_total_count = user_bets_store_unwrapped.bet_keys.len();
        let user_bet_keys_iter = user_bets_store_unwrapped.bet_keys
        .iter()
        .rev()
        .skip((page * page_size) as _)
        .take(page_size as _);

        let user_bet_keys: Vec<&String> = user_bet_keys_iter.collect();
        for bet_key in user_bet_keys { 
            let bet_state: &UserBetStruct = &user_bets_store_unwrapped.bets[bet_key];
            user_bets.push(bet_state.clone());
            if bet_state.tier == 1 {
                let mut round_state = tier1_rounds_store.get_at(bet_state.round_number).unwrap();
                round_state.users_picked_numbers_count = vec![];
                bet_rounds.push(round_state)
            }
            if bet_state.tier == 2 {
                let mut round_state = tier2_rounds_store.get_at(bet_state.round_number).unwrap();
                round_state.users_picked_numbers_count = vec![];
                bet_rounds.push(round_state)
            }
            if bet_state.tier == 3 {
                let mut round_state = tier3_rounds_store.get_at(bet_state.round_number).unwrap();
                round_state.users_picked_numbers_count = vec![];
                bet_rounds.push(round_state)
            }
        }
    }
  
    to_binary(&QueryAnswer::GetPaginatedUserBets {
        user_bets,
        bet_rounds,
        user_bets_total_count
    })
}


fn query_paginated_rounds<S: Storage, A: Api, Q: Querier>(
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
            return to_binary(&QueryAnswer::GetPaginatedRounds {
                tier1_rounds,
                tier2_rounds,
                tier3_rounds
            })
        };

        let rounds_iter = tier1_rounds_store
        .iter()
        .rev()
        .skip((page * page_size) as _)
        .take(page_size as _)
        .map(|x| {
            let mut modified = x.unwrap();
            modified.users_picked_numbers_count = vec![];
            return modified
        });
        
        let rounds: Vec<RoundStruct> = rounds_iter.collect();
        tier1_rounds = Some(rounds);
    }

    if tier2 {
        let tier2_rounds_storage = ReadonlyPrefixedStorage::multilevel(&[ROUNDS_STATE, &"tier2".to_string().as_bytes()], &deps.storage);
        let tier2_rounds_store = if let Some(result) = AppendStore::<RoundStruct, _>::attach(&tier2_rounds_storage) {
            result?
        } else {
            return to_binary(&QueryAnswer::GetPaginatedRounds {
                tier1_rounds,
                tier2_rounds,
                tier3_rounds
            })
        };

        let rounds_iter = tier2_rounds_store
        .iter()
        .rev()
        .skip((page * page_size) as _)
        .take(page_size as _)
        .map(|x| {
            let mut modified = x.unwrap();
            modified.users_picked_numbers_count = vec![];
            return modified
        });

        let rounds: Vec<RoundStruct> = rounds_iter.collect();
        tier2_rounds = Some(rounds);
    }

    if tier3 {
        let tier3_rounds_storage = ReadonlyPrefixedStorage::multilevel(&[ROUNDS_STATE, &"tier3".to_string().as_bytes()], &deps.storage);
        let tier3_rounds_store = if let Some(result) = AppendStore::<RoundStruct, _>::attach(&tier3_rounds_storage) {
            result?
        } else {
            return to_binary(&QueryAnswer::GetPaginatedRounds {
                tier1_rounds,
                tier2_rounds,
                tier3_rounds
            })
        };

        let rounds_iter = tier3_rounds_store
        .iter()
        .rev()
        .skip((page * page_size) as _)
        .take(page_size as _)
        .map(|x| {
            let mut modified = x.unwrap();
            modified.users_picked_numbers_count = vec![];
            return modified
        });

        let rounds: Vec<RoundStruct> = rounds_iter.collect();
        tier3_rounds = Some(rounds);
    }
    
    to_binary(&QueryAnswer::GetPaginatedRounds {
        tier1_rounds,
        tier2_rounds,
        tier3_rounds
    })
}

fn query_rounds <S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    tier1_rounds: Vec<u32>,
    tier2_rounds: Vec<u32>,
    tier3_rounds: Vec<u32>,
) -> StdResult<Binary> {
    let mut rounds : Vec<RoundStruct> = vec![];

    let tier1_rounds_storage = ReadonlyPrefixedStorage::multilevel(&[ROUNDS_STATE, &"tier1".to_string().as_bytes()], &deps.storage);
    let tier1_rounds_store = if let Some(result) = AppendStore::<RoundStruct, _>::attach(&tier1_rounds_storage) {
        result?
    } else {
        return to_binary(&QueryAnswer::GetRounds {
            rounds
        }) 
    };

    for round_number in tier1_rounds {
        let mut round_state: RoundStruct = tier1_rounds_store.get_at(round_number).unwrap();
        round_state.users_picked_numbers_count = vec![];
        rounds.push(round_state)
    }
    
    let tier2_rounds_storage = ReadonlyPrefixedStorage::multilevel(&[ROUNDS_STATE, &"tier2".to_string().as_bytes()], &deps.storage);
    let tier2_rounds_store = if let Some(result) = AppendStore::<RoundStruct, _>::attach(&tier2_rounds_storage) {
        result?
    } else {
        return to_binary(&QueryAnswer::GetRounds {
            rounds
        }) 
    };

    for round_number in tier2_rounds {
        let mut round_state: RoundStruct = tier2_rounds_store.get_at(round_number).unwrap();
        round_state.users_picked_numbers_count = vec![];
        rounds.push(round_state)
    }

    let tier3_rounds_storage = ReadonlyPrefixedStorage::multilevel(&[ROUNDS_STATE, &"tier3".to_string().as_bytes()], &deps.storage);
    let tier3_rounds_store = if let Some(result) = AppendStore::<RoundStruct, _>::attach(&tier3_rounds_storage) {
        result?
    } else {
        return to_binary(&QueryAnswer::GetRounds {
            rounds
        }) 
    };

    for round_number in tier3_rounds {
        let mut round_state: RoundStruct = tier3_rounds_store.get_at(round_number).unwrap();
        round_state.users_picked_numbers_count = vec![];
        rounds.push(round_state)
    }
    
    to_binary(&QueryAnswer::GetRounds {
        rounds
    })
}

fn query_tier_configs<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    tier1: bool,
    tier2: bool,
    tier3: bool
) -> StdResult<Binary> {

    let mut tier1_configs:Option<TierConfig> = None;
    let mut tier2_configs:Option<TierConfig> = None;
    let mut tier3_configs:Option<TierConfig> = None;

    if tier1 {
        let tier_config = ReadonlyPrefixedStorage::new(LUCKY_NUMBER_CONFIG_TIER_1, &deps.storage);
        let entry_fee: Uint128 = load(&tier_config, b"entry_fee")?;
        let triggerer_fee: Uint128 = load(&tier_config, b"triggerer_fee")?;
        let min_entries: i16 = load(&tier_config, b"min_entries")?;
        let max_rand_number: i16 = load(&tier_config, b"max_rand_number")?;

        tier1_configs = Some(TierConfig {
            entry_fee,
            triggerer_fee,
            min_entries,
            max_rand_number
        });
    }

    if tier2 {
        let tier_config = ReadonlyPrefixedStorage::new(LUCKY_NUMBER_CONFIG_TIER_2, &deps.storage);
        let entry_fee: Uint128 = load(&tier_config, b"entry_fee")?;
        let triggerer_fee: Uint128 = load(&tier_config, b"triggerer_fee")?;
        let min_entries: i16 = load(&tier_config, b"min_entries")?;
        let max_rand_number: i16 = load(&tier_config, b"max_rand_number")?;

        tier2_configs = Some(TierConfig {
            entry_fee,
            triggerer_fee,
            min_entries,
            max_rand_number
        });
    }

    if tier3 {
        let tier_config = ReadonlyPrefixedStorage::new(LUCKY_NUMBER_CONFIG_TIER_3, &deps.storage);
        let entry_fee: Uint128 = load(&tier_config, b"entry_fee")?;
        let triggerer_fee: Uint128 = load(&tier_config, b"triggerer_fee")?;
        let min_entries: i16 = load(&tier_config, b"min_entries")?;
        let max_rand_number: i16 = load(&tier_config, b"max_rand_number")?;

        tier3_configs = Some(TierConfig {
            entry_fee,
            triggerer_fee,
            min_entries,
            max_rand_number
        });
    }
    
    to_binary(&QueryAnswer::GetTierConfigs {
        tier1_configs,
        tier2_configs,
        tier3_configs
    })
}

fn query_check_triggers<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<Binary> {
    let mut tier1_trigger: bool = false;
    let mut tier2_trigger: bool = false;
    let mut tier3_trigger: bool = false;

    let tier1_config = ReadonlyPrefixedStorage::new(LUCKY_NUMBER_CONFIG_TIER_1, &deps.storage);
    let min_entries_tier1: i16 = load(&tier1_config, b"min_entries")?;
    let tier1_rounds = ReadonlyPrefixedStorage::multilevel(&[ROUNDS_STATE, &"tier1".to_string().as_bytes()], &deps.storage);
    let tier1_rounds_store = if let Some(result) = AppendStore::<RoundStruct, _>::attach(&tier1_rounds) {
        result?
    } else {
        return to_binary(&QueryAnswer::CheckTriggers {
            tier1_trigger,
            tier2_trigger,
            tier3_trigger
        })
    };
        
    let tier1_cur_round: RoundStruct = tier1_rounds_store.get_at(tier1_rounds_store.len() - 1).unwrap();

    if tier1_cur_round.users_count >= min_entries_tier1 as u32 {
        tier1_trigger = true;
    }

    let tier2_config = ReadonlyPrefixedStorage::new(LUCKY_NUMBER_CONFIG_TIER_2, &deps.storage);
    let min_entries_tier2: i16 = load(&tier2_config, b"min_entries")?;
    let tier2_rounds = ReadonlyPrefixedStorage::multilevel(&[ROUNDS_STATE, &"tier2".to_string().as_bytes()], &deps.storage);
    let tier2_rounds_store = if let Some(result) = AppendStore::<RoundStruct, _>::attach(&tier2_rounds) {
        result?
    } else {
        return to_binary(&QueryAnswer::CheckTriggers {
            tier1_trigger,
            tier2_trigger,
            tier3_trigger
        })
    };
        
    let tier2_cur_round: RoundStruct = tier2_rounds_store.get_at(tier2_rounds_store.len() - 1).unwrap();

    if tier2_cur_round.users_count >= min_entries_tier2 as u32 {
        tier2_trigger = true;
    }

    let tier3_config = ReadonlyPrefixedStorage::new(LUCKY_NUMBER_CONFIG_TIER_3, &deps.storage);
    let min_entries_tier3: i16 = load(&tier3_config, b"min_entries")?;
    let tier3_rounds = ReadonlyPrefixedStorage::multilevel(&[ROUNDS_STATE, &"tier3".to_string().as_bytes()], &deps.storage);
    let tier3_rounds_store = if let Some(result) = AppendStore::<RoundStruct, _>::attach(&tier3_rounds) {
        result?
    } else {
        return to_binary(&QueryAnswer::CheckTriggers {
            tier1_trigger,
            tier2_trigger,
            tier3_trigger
        })
    };
        
    let tier3_cur_round: RoundStruct = tier3_rounds_store.get_at(tier3_rounds_store.len() - 1).unwrap();

    // check if there are enougth pool size (pool_size >= min_entries * entry_fee)
    if tier3_cur_round.users_count >= min_entries_tier3 as u32 {
        tier3_trigger = true;
    }

    to_binary(&QueryAnswer::CheckTriggers {
        tier1_trigger,
        tier2_trigger,
        tier3_trigger
    })
}

fn is_key_valid<S: ReadonlyStorage>(
    storage: &S,
    address: &CanonicalAddr,
    viewing_key: String,
) -> StdResult<bool> {
    // load the address' key
    let read_key = ReadonlyPrefixedStorage::new(PREFIX_VIEW_KEY, storage);
    let load_key: Option<[u8; VIEWING_KEY_SIZE]> = may_load(&read_key, address.as_slice())?;
    let input_key = ViewingKey(viewing_key);
    // if a key was set
    if let Some(expected_key) = load_key {
        // and it matches
        if input_key.check_viewing_key(&expected_key) {
            return Ok(true);
        }
    } else {
        // Checking the key will take significant time. We don't want to exit immediately if it isn't set
        // in a way which will allow to time the command and determine if a viewing key doesn't exist
        input_key.check_viewing_key(&[0u8; VIEWING_KEY_SIZE]);
    }
    Ok(false)
}