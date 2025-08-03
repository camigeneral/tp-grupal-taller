# Variables con valores por defecto
port ?= 4000
service ?=
cmd ?=/bin/bash

.PHONY: client build up stop down logs restart ps exec rm prune rebuild clean

# Ejecutar el cliente en local
client:
	cargo run --bin client $(port)

# Build de todos o un servicio
build:
	@if [ -z "$(service)" ]; then \
		echo "Building all services..."; \
		sudo docker compose build; \
	else \
		echo "Building service: $(service)"; \
		sudo docker compose build $(service); \
	fi

# Levantar todos o un servicio (modo detached)
up:
	@if [ -z "$(service)" ]; then \
		echo "Starting all services..."; \
		sudo docker compose up -d; \
	else \
		echo "Starting service: $(service)"; \
		sudo docker compose up -d $(service); \
	fi

# Ver logs de un servicio
logs:
ifndef service
	$(error Debes pasar service=nombre_del_servicio)
endif
	@echo "Logs de $(service)..."
	@sudo docker compose logs -f $(service)

# Detener todos los servicios
stop:
	sudo docker compose stop

# Bajar y eliminar volúmenes
down:
	sudo docker compose down -v

# Reiniciar todos o uno
restart:
	@if [ -z "$(service)" ]; then \
		echo "Restarting all services..."; \
		sudo docker compose restart; \
	else \
		echo "Restarting service: $(service)"; \
		sudo docker compose restart $(service); \
	fi

# Ver servicios corriendo
ps:
	sudo docker compose ps

# Acceder al contenedor con shell o comando (por defecto /bin/bash)
exec:
ifndef service
	$(error Debes pasar service=nombre_del_servicio)
endif
	@echo "Ingresando a $(service)..."
	@sudo docker compose exec $(service) $(cmd)

# Eliminar contenedores parados
rm:
	sudo docker compose rm -f

# Limpiar todo (containers, networks, volumes, images no usados)
prune:
	sudo docker system prune -af --volumes

# Rebuild forzado
rebuild:
	@echo " Rebuild completo (limpiando cache)..."
	sudo docker compose build --no-cache

# Cliente y servicios definidos como PHONY
clean:
	sudo docker compose down -v --remove-orphans
