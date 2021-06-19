#!/bin/bash
echo Build new contracts to deploy? [yn]
read toBuild

function wait_for_tx() {
  until (secretcli q tx "$1"); do
      sleep 5
  done
}

if [ "$toBuild" != "${toBuild#[Yy]}" ] ;then
    RUST_BACKTRACE=1 cargo unit-test
    rm -f ./contract.wasm ./contract.wasm.gz
    cargo wasm
    cargo schema
    docker run --rm -v $PWD:/contract \
        --mount type=volume,source=factory_cache,target=/code/target \
        --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
        enigmampc/secret-contract-optimizer
fi

secretcli q account $(secretcli keys show -a test1)

deployed=$(secretcli tx compute store "contract.wasm.gz" --from test1 --gas 2000000 -b block -y)
code_id=$(secretcli query compute list-code | jq '.[-1]."id"')
code_hash=$(secretcli query compute list-code | jq '.[-1]."data_hash"')
echo "Stored contract: '$code_id', '$code_hash'"

label=$(date +"%T")
: '
STORE_TX_HASH=$( 
  secretcli tx compute instantiate $code_id " \
  { \
  \"entropy\": 1234,  \
  \"triggerer_address\": \"secret1kw78ltg8380qdrag6puknyk0stdhh4nj68aqj9\",  \
  \"token_address\": \"secret1s7c6xp9wltthk5r6mmavql4xld5me3g37guhsx\",  \
  \"token_hash\": \"CD400FB73F5C99EDBC6AAB22C2593332B8C9F2EA806BF9B42E3A523F3AD06F62\",  \
  \"tier1_entry_fee\": \"10000000\", \
  \"tier1_triggerer_fee\": \"5000000\", \
  \"tier1_min_entries\": 30, \
  \"tier1_max_rand_number\": 30, \
  \"tier2_entry_fee\": \"5000000\", \
  \"tier2_triggerer_fee\": \"2500000\", \
  \"tier2_min_entries\": 15, \
  \"tier2_max_rand_number\": 15, \
  \"tier3_entry_fee\": \"1000000\", \
  \"tier3_triggerer_fee\": \"500000\", \
  \"tier3_min_entries\": 5, \
  \"tier3_max_rand_number\": 5 \
  } \
  " --from test1 --gas 1500000 --label LuckyNumber_$label -b block -y |
  jq -r .txhash
)
'
STORE_TX_HASH=$( 
  secretcli tx compute instantiate $code_id " \
  { \
  \"entropy\": 12419445078009011071,  \
  \"triggerer_address\": \"secret1v5y7as75cqd0trtq62hgzj7u4ck9slhnrf3k4c\",  \
  \"token_address\": \"secret1s7c6xp9wltthk5r6mmavql4xld5me3g37guhsx\",  \
  \"token_hash\": \"CD400FB73F5C99EDBC6AAB22C2593332B8C9F2EA806BF9B42E3A523F3AD06F62\",  \
  \"tier1_entry_fee\": \"10000000\", \
  \"tier1_triggerer_fee\": \"5000000\", \
  \"tier1_min_entries\": 4, \
  \"tier1_max_rand_number\": 4, \
  \"tier2_entry_fee\": \"5000000\", \
  \"tier2_triggerer_fee\": \"2500000\", \
  \"tier2_min_entries\": 3, \
  \"tier2_max_rand_number\": 3, \
  \"tier3_entry_fee\": \"1000000\", \
  \"tier3_triggerer_fee\": \"500000\", \
  \"tier3_min_entries\": 2, \
  \"tier3_max_rand_number\": 2 \
  } \
  " --from test1 --gas 1500000 --label LuckyNumber_$label -b block -y |
  jq -r .txhash
)
wait_for_tx "$STORE_TX_HASH" "Waiting for instantiate to finish on-chain..."

contract_address=$(secretcli query compute list-contract-by-code $code_id | jq '.[-1].address')
echo "contract_address: '$contract_address'"
contract_address_without_quotes=$(echo $contract_address | tr -d '"')

sleep 10

msg=$(base64 -w 0 <<<'{"bet": {"tier": 3, "number": 1}}')
secretcli tx compute execute secret1s7c6xp9wltthk5r6mmavql4xld5me3g37guhsx '{"send":{"recipient": '$contract_address', "amount": "1000000", "msg": "'"$msg"'"}}' --from test1 -y --gas 1500000 -b block

msg=$(base64 -w 0 <<<'{"bet": {"tier": 3, "number": 2}}')
secretcli tx compute execute secret1s7c6xp9wltthk5r6mmavql4xld5me3g37guhsx '{"send":{"recipient": '$contract_address', "amount": "1000000", "msg": "'"$msg"'"}}' --from test2 -y --gas 1500000 -b block

msg=$(base64 -w 0 <<<'{"bet": {"tier": 3, "number": 3}}')
secretcli tx compute execute secret1s7c6xp9wltthk5r6mmavql4xld5me3g37guhsx '{"send":{"recipient": '$contract_address', "amount": "1000000", "msg": "'"$msg"'"}}' --from test3 -y --gas 1500000 -b block

msg=$(base64 -w 0 <<<'{"bet": {"tier": 3, "number": 4}}')
secretcli tx compute execute secret1s7c6xp9wltthk5r6mmavql4xld5me3g37guhsx '{"send":{"recipient": '$contract_address', "amount": "1000000", "msg": "'"$msg"'"}}' --from test4 -y --gas 1500000 -b block

msg=$(base64 -w 0 <<<'{"bet": {"tier": 3, "number": 5}}')
secretcli tx compute execute secret1s7c6xp9wltthk5r6mmavql4xld5me3g37guhsx '{"send":{"recipient": '$contract_address', "amount": "1000000", "msg": "'"$msg"'"}}' --from test5 -y --gas 1500000 -b block

sleep 5

# Get Rounds
#secretcli q compute query $contract_address_without_quotes '{"get_rounds": {"tier1": true, "tier2": true, "tier3": true, "page_size": 10, "page": 0 }}' | base64 --decode --ignore-garbage

# trigger
secretcli tx compute execute $contract_address_without_quotes '{"trigger_lucky_number":{"tier1": true, "tier2": true, "tier3": true, "entropy": 1234}}' --from test1 -y --gas 1500000 -b block

# create VK
#secretcli tx compute execute $contract_address_without_quotes '{"create_viewing_key":{"entropy": "1231231"}}' --from test2 -y --gas 1500000 -b block

# get user bets
#secretcli q compute query $contract_address_without_quotes '{"get_user_bets": {"user_address": "secret1kw78ltg8380qdrag6puknyk0stdhh4nj68aqj9", "viewing_key": "/Sgx2v+2e/IJ7eIJcdRLbDg91mz3be6ZjuA0lTqWLdM="}}' | base64 --decode --ignore-garbage

# withdraw
#secretcli tx compute execute $contract_address_without_quotes '{"withdrawl":{"tier": 3, "round": 0}}' --from test2 -y --gas 1500000 -b block

#secretcli q compute query secret1v9w7798n4dv9rphcl6983az53ywzrzwtuzz8ry '{"get_user_bets": {"user_address": "secret1kw78ltg8380qdrag6puknyk0stdhh4nj68aqj9"}}' | base64 --decode --ignore-garbage

#secretcli q compute query secret1vkrgphn45944uekp7fn9hf5qzgzxpvq2h0mern '{"get_rounds": {"tier1": true, "tier2": true, "tier3": true, "page_size": 10, "page": 0 }}' | base64 --decode --ignore-garbage

#secretcli tx compute execute secret1m4ez6yw8yr68fv8a8p744qqqpug6plnx9d4eny '{"withdrawl":{"tier": 3, "round": 0}}' --from test1 -y --gas 1500000 -b block

#msg=$(base64 -w 0 <<<'{"bet": {"tier": 3, "number": 1}}')
#secretcli tx compute execute secret1s7c6xp9wltthk5r6mmavql4xld5me3g37guhsx '{"send":{"recipient": "secret1nfycy4kdphekvdd75nvgu535wjg5jwjdakq5ec", "amount": "1000000", "msg": "'"$msg"'"}}' --from test1 -y --gas 1500000 -b block

#secretcli q compute query  secret1s7c6xp9wltthk5r6mmavql4xld5me3g37guhsx '{"balance":{"address": "secret1kw78ltg8380qdrag6puknyk0stdhh4nj68aqj9", "key": "api_key_IwYF2GwgPAawIp7JgJJAJKE7uW/Sj/VVJDodcOSWsZQ="}}'

#secretcli tx compute execute secret10jzqsnqm88nlxzvpq3c8feg7yaft7a73np8t5v '{"trigger_lucky_number":{"tier1": true, "tier2": true, "tier3": true, "entropy": 1234}}' --from test1 -y --gas 1500000 -b block

secretcli q compute query secret10jzqsnqm88nlxzvpq3c8feg7yaft7a73np8t5v '{"check_triggers":{}}' | base64 --decode --ignore-garbage

#secretcli tx compute execute secret1s7c6xp9wltthk5r6mmavql4xld5me3g37guhsx '{"redeem":{"amount": "1000000"}}' --from test1 -y --gas 1500000 -b block

#secretcli tx compute execute secret1jhcvug7afjyqfuf465p7vf900rvl838nwnc04u '{"change_triggerer":{"triggerer": "secret1kw78ltg8380qdrag6puknyk0stdhh4nj68aqj9"}}' --from test1 -y --gas 1500000 -b block
#secretcli tx compute execute secret1jhcvug7afjyqfuf465p7vf900rvl838nwnc04u '{"change_admin":{"admin": "secret1kw78ltg8380qdrag6puknyk0stdhh4nj68aqj9"}}' --from test1 -y --gas 1500000 -b block
#secretcli tx compute execute secret1jhcvug7afjyqfuf465p7vf900rvl838nwnc04u '{"change_tier":{"tier": 1,"entry_fee": "30000000", "triggerer_fee": "15000000", "min_entries": 30, "max_rand_number": 30}}' --from test1 -y --gas 1500000 -b block
