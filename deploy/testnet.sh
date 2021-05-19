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

secretcli q account $(secretcli keys show -a a)

deployed=$(secretcli tx compute store "contract.wasm.gz" --from b --gas 2000000 -b block -y)
code_id=$(secretcli query compute list-code | jq '.[-1]."id"')
code_hash=$(secretcli query compute list-code | jq '.[-1]."data_hash"')
echo "Stored contract: '$code_id', '$code_hash'"

label=$(date +"%T")
deployer_name_a=b
    
STORE_TX_HASH=$( 
  secretcli tx compute instantiate $code_id " \
  { \
  \"entropy\": 1234,  \
  \"triggerer_address\": \"secret1ypfxpp4ev2sd9vj9ygmsmfxul25xt9cfadrxxy\",  \
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
  " --from $deployer_name_a --gas 1500000 --label LuckyNumber_$label -b block -y |
  jq -r .txhash
)
wait_for_tx "$STORE_TX_HASH" "Waiting for instantiate to finish on-chain..."

contract_address=$(secretcli query compute list-contract-by-code $code_id | jq '.[-1].address')
echo "contract_address: '$contract_address'"