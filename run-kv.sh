#!/bin/bash

PEER_ADDR=""
for ((i=0; i<TOTAL_REPLICAS; i++)); do
    # ping the server to check if it is ready
    CANDIDATE_HOSTNAME="kv-server-$i.kv-server.default.svc.cluster.local"
    # DNS lookup the candidate hostname until it is ready
    while true; do
        result="nslookup $CANDIDATE_HOSTNAME"
        if [[ $result == *"can't find"* ]]; then
            echo "Haven't found $CANDIDATE_HOSTNAME yet, sleep for 1 second"
            sleep 1
        else
            echo "Found $CANDIDATE_HOSTNAME:"
            echo "$result"
            break
        fi
    done
    if [ -z "$PEER_ADDR" ]; then
        PEER_ADDR="$CANDIDATE_HOSTNAME:$RAFT_PORT"
    else
        PEER_ADDR="$PEER_ADDR $CANDIDATE_HOSTNAME:$RAFT_PORT"
    fi
done
export PEER_ADDR
export SELF_ADDR="$SELF_HOSTNAME.kv-server.default.svc.cluster.local:$RAFT_PORT"
# sleep for 10 seconds to wait for all pods to be ready
#sleep 10

./storgata-db