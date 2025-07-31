# Número de instancias por defecto si no se especifica
port ?= 4000

client: 
	cargo run --bin client $(port)


.PHONY: client clean
