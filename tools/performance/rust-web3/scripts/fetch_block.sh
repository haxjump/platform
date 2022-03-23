#!/usr/bin/env bash

set -e

start=$1
count=$2

endpoint="https://dev-qa01.dev.findora.org"
port=26657

check_error() {
  error=$(echo "$1" | jq -r '.error')
  if [ "$error" != "null" ]; then
    return 1
  fi
  return 0
}

fetch_block() {
  height=$1
  lt=$2
  block=$(curl -s -X GET "$endpoint:$port/block?height=$height" -H "accept: application/json")
  check_error "$block" || return 1
  h=$(echo "$block" | jq -r '.result.block.header.height')
  t0=$(echo "$block" | jq -r '.result.block.header.time')
  c=$(echo "$block" | jq -r '.result.block.data.txs | length')
  t1=$(jq -n "\"$t0\" | sub(\".[0-9]+Z$\"; \"Z\") | fromdate")
  if ((lt==0)); then
    bt=0
  else
    bt=$((t1-lt))
  fi
  echo "$h,$t1,$c,$bt"
}

block_timestamp() {
  height=$1
  block=$(curl -s -X GET "$endpoint:$port/block?height=$height" -H "accept: application/json")
  check_error "$block" || return 1
  t0=$(echo "$block" | jq -r '.result.block.header.time')
  t1=$(jq -n "\"$t0\" | sub(\".[0-9]+Z$\"; \"Z\") | fromdate")
  echo "$t1"
}

latest_height=$(curl -s "$endpoint:$port/status" | jq -r .result.sync_info.latest_block_height)

height=$start
if ((start == 0)); then
  last=0;
else
  last_height=$((start-1))
  if ! last=$(block_timestamp $last_height); then
    echo "Cannot obtain last block $last_height, latest height $latest_height"
    exit 1
  fi
fi

while :
do
  block=$(fetch_block "$height" "$last")
  last=$(echo "$block" | awk -F',' '{print $2}')
  echo "$block"

  ((height+=1))
  if ((height >= latest_height || (count != 0 && height >= start+count)))
  then
    break
  fi
done
