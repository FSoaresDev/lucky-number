.PHONY: check
check:
	cargo check

.PHONY: clippy
clippy:
	cargo clippy

PHONY: test
test: unit-test

.PHONY: unit-test
unit-test:
	cargo test

.PHONY: compress-wasm
compress-wasm:
	cp ./target/wasm32-unknown-unknown/release/*.wasm ./contract.wasm
	@## The following line is not necessary, may work only on linux (extra size optimization)
	@# wasm-opt -Os ./contract.wasm -o ./contract.wasm
	cat ./contract.wasm | gzip -9 > ./contract.wasm.gz

.PHONY: schema
schema:
	cargo run --example schema

# Run local development chain with four funded accounts (named a, b, c, and d)
.PHONY: start-server
start-server: # CTRL+C to stop
	docker run -it --rm \
		-p 26657:26657 -p 26656:26656 -p 1317:1317 \
		-v $$(pwd):/root/code \
		--name secretdev enigmampc/secret-network-sw-dev:v1.0.4-3

.PHONY: build-store-contract
build-store-contract:
	cargo clean
	-rm -f ./contract.wasm ./contract.wasm.gz
	RUSTFLAGS='-C link-arg=-s' cargo build --release --target wasm32-unknown-unknown
	cp ./target/wasm32-unknown-unknown/release/*.wasm ./contract.wasm
	cat ./contract.wasm | gzip -9 > ./contract.wasm.gz
	docker exec secretdev secretcli tx compute store -y --from a --gas 10000000 /root/code/contract.wasm.gz

.PHONY: list-code
list-code:
	docker exec secretdev secretcli query compute list-code

#make instanciate-contract CODE=1 TRIGGERER=secret1ypfxpp4ev2sd9vj9ygmsmfxul25xt9cfadrxxy
.PHONY: instanciate-contract
instanciate-contract:
	docker exec secretdev bash -c "\
	secretcli tx compute instantiate $(CODE) \
	'{\
		\"entropy\": 123, \
		\"triggerer_address\": \"$(TRIGGERER)\", \
		\"token_address\": \"secret1ypfxpp4ev2sd9vj9ygmsmfxul25xt9cfadrxxy\", \
		\"token_hash\": \"0xb66c6aca95004916baa13f8913ff1222c3e1775aaaf60f011cfaba7296d59d2c\", \
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
	}' \
	--from a --gas 1500000 --label $(CODE) -b block -y \
	"

#make trigger CONTRACT=secret1hzdlry39ydm0wqflglslcu26v6dnxzk0dnttf9
.PHONY: trigger
trigger:
	docker exec secretdev bash -c "\
	secretcli tx compute execute $(CONTRACT) '{\"trigger_lucky_number\": {\"tier1\": true, \"tier2\": true, \"tier3\": true, \"entropy\": 52651}}' \
	--from a --gas 1500000 -b block -y \
	"

#make get-triggerer CONTRACT=secret16t7y0vrtpqjw2d7jvc2209yan9002339gndv93
.PHONY: get-triggerer
get-triggerer:
	docker exec secretdev bash -c "secretcli q compute query $(CONTRACT) '{\"get_triggerer\": {}}' | base64 --decode --ignore-garbage"

#make get-rounds CONTRACT=secret1grmyj2j670w6e9psjwue8r3f3ezk37ashmcy8f
.PHONY: get-rounds
get-rounds:
	docker exec secretdev bash -c "secretcli q compute query $(CONTRACT) '{\"get_rounds\": {\"tier1\": true, \"tier2\": true, \"tier3\": true, \"page_size\": 10, \"page\": 0 }}'| base64 --decode --ignore-garbage"

#make hashes TX=99548FEB8D07C75E475814CA5A6FAD707893D80198E28A01FA1898C8D0FFCA4E
.PHONY: hashes
hashes:
	docker exec secretdev secretcli query compute tx $(TX)
	
	
.PHONY: deploy
deploy:
	bash deploy/testnet.sh