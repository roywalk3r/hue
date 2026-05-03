.PHONY: all start stop build test lint clean

all: start

build:
	cd huerust && cargo build --release
	cd node-scraper && npm ci

start:
	docker compose up -d

stop:
	docker compose down

test:
	cd huerust && cargo test
	./tests/contract/test-node-scraper-api.sh

lint:
	cd huerust && cargo clippy -- -D warnings 2>/dev/null || cargo check
	npm run lint

clean:
	cd huerust && cargo clean
	docker compose down -v
