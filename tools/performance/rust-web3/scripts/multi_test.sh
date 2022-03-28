#!/usr/bin/env bash

set -e

test_endpoint="http://dev-qa01-us-west-2-full-000-open.dev.findora.org:8545"

cd "$script_dir"/..

#./feth --network "$test_endpoint" --count 1000 --max-parallelism 10 --timeout 100 >> test.log 2>&1
#echo "wait for 60 seconds to make sure all previous txns have been finished..." >> test.log
#sleep 60

#./feth --network "$test_endpoint" --count 1000 --max-parallelism 15 --timeout 100 >> test.log 2>&1
#echo "wait for 60 seconds to make sure all previous txns have been finished..." >> test.log
#sleep 60

#./feth --network "$test_endpoint" --count 1000 --max-parallelism 20 --timeout 100 >> test.log 2>&1
#echo "wait for 60 seconds to make sure all previous txns have been finished..." >> test.log
#sleep 60
#
#./feth --network "$test_endpoint" --count 3000 --max-parallelism 25 --timeout 100 >> test.log 2>&1
#echo "wait for 60 seconds to make sure all previous txns have been finished..." >> test.log
#sleep 60
#
#./feth --network "$test_endpoint" --count 1000 --max-parallelism 30 --timeout 100 >> test.log 2>&1
#echo "wait for 60 seconds to make sure all previous txns have been finished..." >> test.log
#sleep 60
#
#./feth --network "$test_endpoint" --count 1000 --max-parallelism 35 --timeout 100 >> test.log 2>&1
#echo "wait for 60 seconds to make sure all previous txns have been finished..." >> test.log
#sleep 60
#
#./feth --network "$test_endpoint" --count 1000 --max-parallelism 40 --timeout 100 >> test.log 2>&1
#echo "wait for 60 seconds to make sure all previous txns have been finished..." >> test.log
#sleep 60

./feth --network "$test_endpoint" --count 1000 --max-parallelism 45 --timeout 100 >> test.log 2>&1
echo "wait for 60 seconds to make sure all previous txns have been finished..." >> test.log
sleep 60

./feth --network "$test_endpoint" --count 1000 --max-parallelism 48 --timeout 100 >> test.log 2>&1
echo "wait for 60 seconds to make sure all previous txns have been finished..." >> test.log
sleep 60

./feth --network "$test_endpoint" --count 1000 --max-parallelism 50 --timeout 100 >> test.log 2>&1
echo "wait for 60 seconds to make sure all previous txns have been finished..." >> test.log
sleep 60

./feth --network "$test_endpoint" --count 1000 --max-parallelism 52 --timeout 100 >> test.log 2>&1
echo "wait for 60 seconds to make sure all previous txns have been finished..." >> test.log
sleep 60

./feth --network "$test_endpoint" --count 1000 --max-parallelism 55 --timeout 100 >> test.log 2>&1
echo "wait for 60 seconds to make sure all previous txns have been finished..." >> test.log
sleep 60

cd -
