#!/usr/bin/env bash

ENV=dev
NAMESPACE=qa01
SERV_URL=https://${ENV}-${NAMESPACE}.${ENV}.findora.org
IMG_PREFIX='public.ecr.aws/k6m5b6e2/release/findorad'

check_env() {
  ((START_MODE != 2)) || return 1
  for i in wget curl jq; do
    if ! which $i >/dev/null 2>&1; then
      echo -e "\n\033[31;01m${i}\033[00m has not been installed properly!\n"
      exit 1
    fi
  done
}

check_env

# default: syncing from block 0
START_MODE=1
# image version
VER=$2
# root directory
ROOT_DIR=$3

if [ "$1" = "snapshot" ]; then
    START_MODE=0
elif [ "$1" = "restart" ]; then
  START_MODE=2
else
    sudo rm -rf \
    "$ROOT_DIR"/findorad \
    "$ROOT_DIR"/tendermint
fi

if [ -n "$VER" ]; then
  NODE_IMG="$IMG_PREFIX:$VER"
elif VER=$(curl -s $SERV_URL:8668/version); then
  if VER=$(echo "$VER" | awk '{print $2}'); then
    NODE_IMG="$IMG_PREFIX:$VER"
  else
    echo "Invalid image version"
    exit 2
  fi
else
  echo "Failed to obtain image version"
  exit 1
fi

[ -n "$ROOT_DIR" ] || ROOT_DIR=/data/findora/$NAMESPACE

if ChainID=$(curl -s -X POST -H "Content-Type: application/json" \
     -d '{"jsonrpc":"2.0","method":"eth_chainId","id":1}' \
     "$SERV_URL:8545"); then
    HEX=$(echo "$ChainID" | jq -r .result |awk -F'x' '{print $2}')
    if ! EVM_CHAIN_ID=$(echo "obase=10; ibase=16; $HEX" | bc); then
      echo "Invalid Evm chain id"
      exit 2
    fi
else
  echo "Failed to obtain chain id"
  exit 2
fi

echo "using image $IMG"
echo "root directory $ROOT_DIR"
echo "chain id $EVM_CHAIN_ID"

######################
# Config local node #
######################
if ((START_MODE != 2)); then
  # clean old data and config files
  sudo rm -rf ${ROOT_DIR}/findorad || exit 1
  docker run --rm -v ${ROOT_DIR}/tendermint:/root/.tendermint "${NODE_IMG}" init --${NAMESPACE}|| exit 1
  sudo chown -R "$(id -u)":"$(id -g)" ${ROOT_DIR}/tendermint/
fi

if ((START_MODE == 1)); then
    mkdir -p ${ROOT_DIR}/tendermint/config
    mkdir -p ${ROOT_DIR}/tendermint/data
    
    cp -f ${ROOT_DIR}/../$NAMESPACE.config.toml ${ROOT_DIR}/tendermint/config/config.toml
    cp -f ${ROOT_DIR}/../zero_height.json ${ROOT_DIR}/tendermint/data/priv_validator_state.json

    rm -rf "${ROOT_DIR}/findorad"
    rm -rf "${ROOT_DIR}/tendermint/config/addrbook.json"
elif ((START_MODE == 0)); then
    ###################
    # get snapshot    #
    ###################
    # download latest link and get url
    wget -O "${ROOT_DIR}/latest" "https://${ENV}-${NAMESPACE}-us-west-2-chain-data-backup.s3.us-west-2.amazonaws.com/latest"
    CHAINDATA_URL=$(cut -d , -f 1 "${ROOT_DIR}/latest")
    echo "$CHAINDATA_URL"
    
    # remove old data 
    rm -rf "${ROOT_DIR}/findorad"
    rm -rf "${ROOT_DIR}/tendermint/data"
    rm -rf "${ROOT_DIR}/tendermint/config/addrbook.json"
    
    wget -O "${ROOT_DIR}/snapshot" "${CHAINDATA_URL}" 
    mkdir "${ROOT_DIR}/snapshot_data"
    echo "extracting snapshot data..."
    tar zxf "${ROOT_DIR}/snapshot" -C "${ROOT_DIR}/snapshot_data"

    echo "moving data to right place..."
    mv "${ROOT_DIR}/snapshot_data/data/ledger" "${ROOT_DIR}/findorad"
    mv "${ROOT_DIR}/snapshot_data/data/tendermint/mainnet/node0/data" "${ROOT_DIR}/tendermint/data"
    
    rm -rf ${ROOT_DIR}/snapshot_data
fi

###################
# Run local node #
###################

docker rm -f findorad || exit 1
docker run -d \
    -v ${ROOT_DIR}/tendermint:/root/.tendermint \
    -v ${ROOT_DIR}/findorad:/tmp/findora \
    -p 8669:8669 \
    -p 8668:8668 \
    -p 8667:8667 \
    -p 26657:26657 \
    -p 8545:8545 \
    -e EVM_CHAIN_ID="$EVM_CHAIN_ID" \
    -e RUST_LOG="abciapp=info,baseapp=debug,account=debug,ethereum=debug,evm=info,eth_rpc=debug" \
    --name findorad \
    "$NODE_IMG" node \
    --ledger-dir /tmp/findora \
    --tendermint-host 0.0.0.0 \
    --tendermint-node-key-config-path="/root/.tendermint/config/priv_validator_key.json" \
    --enable-query-service \
    --enable-eth-api-service

sleep 10

curl -s 'http://localhost:26657/status' | jq -r .result.node_info.network
curl -s 'http://localhost:8668/version'; echo
curl -s -X POST -H "Content-Type: application/json" \
     -d '{"jsonrpc":"2.0","method":"eth_hashrate","id":1}' \
     'http://localhost:8545'; echo

echo "Local node initialized, syncing is running in background."
