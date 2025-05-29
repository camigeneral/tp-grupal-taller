redis: 
	cargo run --bin redis_server 4000

microservice:
	cargo run --bin microservice

client: 
	cargo run --bin client

clean:
	rm -rf target