# Variables con valores por defecto
port ?= 4000
service ?=
cmd ?=/bin/sh

.PHONY: client build up stop down logs restart ps exec rm prune rebuild clean

client:
	cargo run --bin client $(port)

build:
	@if [ -z "$(service)" ]; then \
		echo "Building all services..."; \
		sudo docker compose build; \
	else \
		echo "Building service: $(service)"; \
		sudo docker compose build $(service); \
	fi

up:
	@if [ -z "$(service)" ]; then \
		echo "Starting all services..."; \
		sudo docker compose up -d; \
	else \
		echo "Starting service: $(service)"; \
		sudo docker compose up -d $(service); \
	fi

logs:
	@if [ -z "$(service)" ]; then \
		echo "Error: Debes pasar service=nombre_del_servicio"; \
		exit 1; \
	fi
	@echo "Logs de $(service)..."
	sudo docker compose logs -f $(service)

stop:
	@if [ -z "$(service)" ]; then \
		echo "Stopping all services..."; \
		sudo docker compose stop; \
	else \
		echo "Stopping service: $(service)"; \
		sudo docker compose stop $(service); \
	fi

down:
	sudo docker compose down -v

restart:
	@if [ -z "$(service)" ]; then \
		echo "Restarting all services..."; \
		sudo docker compose restart; \
	else \
		echo "Restarting service: $(service)"; \
		sudo docker compose restart $(service); \
	fi

clean_build:
	sudo docker compose down -v --rmi all --remove-orphans
	sudo docker image prune -f
	sudo docker compose build --no-cache

ps:
	sudo docker compose ps

exec:
	@if [ -z "$(service)" ]; then \
		echo "Error: Debes pasar service=nombre_del_servicio"; \
		exit 1; \
	fi
	@echo "Ingresando a $(service)..."
	sudo docker compose exec $(service) $(cmd)

rm:
	sudo docker compose rm -f

prune:
	@echo "Limpiando imágenes no usadas solo del proyecto..."
	sudo docker image prune -f
	sudo docker rmi llm_microservice:latest redis-node:latest microservice:latest || true

rebuild:
	@echo "Rebuild completo (limpiando cache)..."
	sudo docker compose build --no-cache

clean:
	sudo docker compose down -v --remove-orphans
