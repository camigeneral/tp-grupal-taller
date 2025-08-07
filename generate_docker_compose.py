import yaml

node_config = [
    {"name": "node0", "rdb": "redis_node_0_5460_14000.rdb", "port": 4000},
    {"name": "node1", "rdb": "redis_node_5460_10921_14001.rdb", "port": 4001},
    {"name": "node2", "rdb": "redis_node_10921_16383_14002.rdb", "port": 4002},
    {"name": "node3", "rdb": "redis_node_0_5460_14003.rdb", "port": 4003},
    {"name": "node4", "rdb": "redis_node_0_5460_14004.rdb", "port": 4004},
    {"name": "node5", "rdb": "redis_node_5460_10921_14005.rdb", "port": 4005},
    {"name": "node6", "rdb": "redis_node_5460_10921_14006.rdb", "port": 4006},
    {"name": "node7", "rdb": "redis_node_10921_16383_14007.rdb", "port": 4007},
    {"name": "node8", "rdb": "redis_node_10921_16383_14008.rdb", "port": 4008},
]

services = {}
network_name = "redinternanodos"

for node in node_config:
    node_name = node["name"]
    port = node["port"]
    log_file = f"./logs/{node_name}.log"
    rdb_file = f"./redis_server/rdb_files/{node['rdb']}"

    services[node_name] = {
        "networks": [network_name],
        "build": {
            "context": ".",
            "dockerfile": "redis_server/Dockerfile",
        },
        "image": "redis-node",
        "container_name": node_name,
        "working_dir": "/app/",
        "environment": {
            "LOG_FILE": f"/app/logs/{node_name}.log"
        },
        "ports": [
            f"{14000 + (port - 4000)}:{14000 + (port - 4000)}",
            f"{port}:{port}"
        ],
        "volumes": [
            f"{rdb_file}:/app/redis_server/rdb_files/{node['rdb']}",
            f"{log_file}:/app/logs/{node_name}.log"
        ],
        "ulimits": {
            "nofile": {
                "soft": 65536,
                "hard": 65536
            }
        },
        "command": ["/app/redis_server_bin", str(port)]
    }

services["llm_microservice"] = {
    "networks": [network_name],
    "build": {
        "dockerfile": "llm_microservice/Dockerfile"
    },
    "restart": "on-failure",
    "image": "llm_microservice",
    "container_name": "llm_microservice",
    "working_dir": "/app",
    "volumes": [
        "./logs/llm_microservice.log:/app/logs/llm_microservice.log"
    ],
    "depends_on": [n["name"] for n in node_config],
    "environment": {
        "REDIS_NODE_HOSTS": ",".join([f"{n['name']}:{n['port']}" for n in node_config[::-1]]),
        "GEMINI_API_KEY": "${GEMINI_API_KEY}",
        "LOG_FILE": "/app/logs/llm_microservice.log"
    },
    "ports": ["4030:4030"],
    "command": ["/app/llm_microservice_bin"]
}

services["microservice"] = {
    "networks": [network_name],
    "build": {
        "context": ".",
        "dockerfile": "microservice/Dockerfile"
    },
    "restart": "on-failure",
    "image": "microservice",
    "container_name": "microservice",
    "working_dir": "/app",
    "volumes": [
        "./logs/microservice.log:/app/logs/microservice.log"
    ],
    "depends_on": [n["name"] for n in node_config],
    "environment": {
        "REDIS_NODE_HOSTS": ",".join([f"{n['name']}:{n['port']}" for n in node_config[::-1]]),
        "LOG_FILE": "/app/logs/microservice.log"
    },
    "ports": ["5000:5000"],
    "command": ["/app/microservice_bin"]
}

docker_compose = {
    "networks": {
        network_name: {
            "driver": "bridge"
        }
    },
    "services": services
}

with open("docker-compose.yml", "w") as f:
    yaml.dump(docker_compose, f, sort_keys=False)
