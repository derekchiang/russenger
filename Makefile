LIB_PATH ?= -L lib -L ../rust-msgpack/lib

build:
	mkdir -p lib bin
	rustc -O $(LIB_PATH) --out-dir lib src/russenger/lib.rs
	rustc -O $(LIB_PATH) -o bin/example src/example/main.rs

clean:
	rm -rf lib bin

.PHONY: build clean