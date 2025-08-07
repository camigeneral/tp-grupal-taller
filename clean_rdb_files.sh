FILES=(
  "redis_server/rdb_files/redis_node_0_5460_14000.rdb"
  "redis_server/rdb_files/redis_node_5460_10921_14001.rdb"
  "redis_server/rdb_files/redis_node_10921_16383_14002.rdb"
  "redis_server/rdb_files/redis_node_0_5460_14003.rdb"
  "redis_server/rdb_files/redis_node_0_5460_14004.rdb"
  "redis_server/rdb_files/redis_node_5460_10921_14005.rdb"
  "redis_server/rdb_files/redis_node_5460_10921_14006.rdb"
  "redis_server/rdb_files/redis_node_10921_16383_14007.rdb"
  "redis_server/rdb_files/redis_node_10921_16383_14008.rdb"
)

for file in "${FILES[@]}"; do
    if [ -f "$file" ]; then
        > "$file"
    else
        echo "file not found: $file"
    fi
done
