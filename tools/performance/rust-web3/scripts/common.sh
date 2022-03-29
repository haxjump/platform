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

latest_block() {
  block=$(curl -s -X GET "$endpoint:$port/block" -H "accept: application/json")
  check_error "$block" || return 1
}

line_of() {
  file=$1
  wc -l "$file" |awk '{print $1}'
}

