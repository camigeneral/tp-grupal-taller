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
		docker compose build; \
	else \
		echo "Building service: $(service)"; \
		docker compose build $(service); \
	fi

up:
	@if [ -z "$(service)" ]; then \
		echo "Starting all services..."; \
		docker compose up -d; \
	else \
		echo "Starting service: $(service)"; \
		docker compose up -d $(service); \
	fi

logs:
	@if [ -z "$(service)" ]; then \
		echo "Error: Debes pasar service=nombre_del_servicio"; \
		exit 1; \
	fi
	@echo "Logs de $(service)..."
	docker compose logs -f $(service)

stop:
	@if [ -z "$(service)" ]; then \
		echo "Stopping all services..."; \
		docker compose stop; \
	else \
		echo "Stopping service: $(service)"; \
		docker compose stop $(service); \
	fi

down:
	docker compose down -v

restart:
	@if [ -z "$(service)" ]; then \
		echo "Restarting all services..."; \
		docker compose restart; \
	else \
		echo "Restarting service: $(service)"; \
		docker compose restart $(service); \
	fi

clean_build:
	docker compose down -v --rmi all --remove-orphans
	docker image prune -f
	docker compose build --no-cache

ps:
	docker compose ps

exec:
	@if [ -z "$(service)" ]; then \
		echo "Error: Debes pasar service=nombre_del_servicio"; \
		exit 1; \
	fi
	@echo "Ingresando a $(service)..."
	docker compose exec $(service) $(cmd)

rm:
	docker compose rm -f

prune:
	@echo "Limpiando imágenes no usadas solo del proyecto..."
	docker image prune -f
	docker rmi llm_microservice:latest redis-node:latest microservice:latest || true

rebuild:
	@echo "Rebuild completo (limpiando cache)..."
	docker compose build --no-cache

clean:
	docker compose down -v --remove-orphans
