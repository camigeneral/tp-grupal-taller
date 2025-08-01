# Número de instancias por defecto si no se especifica
port ?= 4000

client: 
	cargo run --bin client $(port)

build:
	sudo docker compose build

up:
	sudo docker compose up

stop:
	sudo docker compose stop

down:
	sudo docker compose down -v


.PHONY: client clean
