# Número de instancias por defecto si no se especifica
port ?= 4000
service ?=
client: 
	cargo run --bin client $(port)

build:
	sudo docker compose build

up:
	sudo docker compose up -d

stop:
	sudo docker compose stop

logs: 
	sudo docker compose logs -f $(service)

down:
	sudo docker compose down -v


.PHONY: client clean
