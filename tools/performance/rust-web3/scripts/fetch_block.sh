#!/usr/bin/env bash

set -e

start=$1
count=$2

endpoint="https://dev-qa01.dev.findora.org"
port=26657

script_dir=$(dirname "$0")
. "$script_dir"/common.sh

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
