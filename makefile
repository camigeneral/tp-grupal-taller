# Número de instancias por defecto si no se especifica
nodes ?= 1

redis:
	@if [ $(nodes) -lt 1 ] || [ $(nodes) -gt 3 ]; then \
		echo "Error: El número de instancias debe estar entre 1 y 3"; \
		exit 1; \
	fi; \
	for i in $$(seq 1 $(nodes)); do \
		PORT=$$((4000 + $$i - 1)); \
		echo "Iniciando Redis Server $$i en puerto $$PORT"; \
		cargo run --bin redis_server $$PORT & \
	done

microservice:
	cargo run --bin microservice

client: 
	cargo run --bin client


clean_redis:
	pkill -f "redis_server" || true

clean:
	rm -rf target
	pkill -f "redis_server" || true

.PHONY: redis microservice client clean