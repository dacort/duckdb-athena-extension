.PHONY: all clean build release update

all: release

OSX_BUILD_UNIVERSAL_FLAG=
ifeq (${OSX_BUILD_UNIVERSAL}, 1)
	OSX_BUILD_UNIVERSAL_FLAG=-DOSX_BUILD_UNIVERSAL=1
endif

ifeq ($(GEN),ninja)
	GENERATOR=-G "Ninja"
	FORCE_COLOR=-DFORCE_COLORED_OUTPUT=1
endif

BUILD_FLAGS=-DEXTENSION_STATIC_BUILD=1  -DCLANG_TIDY=False ${OSX_BUILD_UNIVERSAL_FLAG}

clean:
	rm -rf build
	cargo clean

# Debug build
build:
	mkdir -p build/debug && \
	cd build/debug && \
	cmake $(GENERATOR) $(FORCE_COLOR) -DCMAKE_BUILD_TYPE=Debug ${BUILD_FLAGS} ../../duckdb/CMakeLists.txt -DEXTERNAL_EXTENSION_DIRECTORIES=../../duckdb-athena-extension -B. && \
	cmake --build . --config Debug -j


release:
	mkdir -p build/release && \
	cd build/release && \
	cmake $(GENERATOR) $(FORCE_COLOR) -DCMAKE_BUILD_TYPE=Release ${BUILD_FLAGS} \
		../../duckdb/CMakeLists.txt -DEXTERNAL_EXTENSION_DIRECTORIES=../../duckdb-athena-extension -B. && \
	cmake --build . --config Release


update:
	git submodule update --remote --merge