#!/bin/bash

FILES=("redis_server/rdb_files/redis_node_0_5460_14000.rdb" "redis_server/rdb_files/redis_node_5460_10921_14001.rdb" "redis_server/rdb_files/redis_node_10921_16383_14002.rdb"
  "redis_server/rdb_files/redis_node_0_5460_14003.rdb"
  "redis_server/rdb_files/redis_node_0_5460_14004.rdb"
  "redis_server/rdb_files/redis_node_5460_10921_14005.rdb"
  "redis_server/rdb_files/redis_node_5460_10921_14006.rdb"
  "redis_server/rdb_files/redis_node_10921_16383_14007.rdb"
  "redis_server/rdb_files/redis_node_10921_16383_14008.rdb"
  "logs/node0.log"
  "logs/node1.log"
  "logs/node2.log"
  "logs/node3.log"
  "logs/node4.log"
  "logs/node5.log"
  "logs/node6.log"
  "logs/node7.log"
  "logs/node8.log"
  "logs/llm_microservice.log"
  "logs/microservice.log")

for file in "${FILES[@]}"; do
    if [ -f "$file" ]; then
        > "$file"
    else
        echo "file not found: $file"
    fi
done
