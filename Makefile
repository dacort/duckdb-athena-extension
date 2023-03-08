.PHONY: all clean build release update

all: release

OSX_BUILD_UNIVERSAL_FLAG=
ifeq (${OSX_BUILD_UNIVERSAL}, 1)
	OSX_BUILD_UNIVERSAL_FLAG=-DOSX_BUILD_UNIVERSAL=1
endif

# On Github, if we want to cross-compile, we have to explicitly set the target architecture.
# This is because the corrosion-rs thing that's used doesn't support universal binaries.
# The only way I've seen folks do it is by using the (unmaintained) cargo-lipo or
# by building each target and using lipo manually as done here:
# https://github.com/walles/riff/blob/82f77c82e7306dd69d343640670bdf9d31cc0b0b/release.sh#L132-L136
# For us, we'll create an ARM64 flag we can use as the default target in GitHub mac runners is intel
OSX_BUILD_AARCH64_FLAG=
ifeq (${OSX_BUILD_AARCH64}, 1)
	OSX_BUILD_AARCH64_FLAG=-DOSX_BUILD_AARCH64=1
endif

ifeq ($(GEN),ninja)
	GENERATOR=-G "Ninja"
	FORCE_COLOR=-DFORCE_COLORED_OUTPUT=1
endif

BUILD_FLAGS=-DEXTENSION_STATIC_BUILD=1  -DCLANG_TIDY=False ${OSX_BUILD_UNIVERSAL_FLAG} ${OSX_BUILD_AARCH64_FLAG}

clean:
	rm -rf build
	cargo clean

# Debug build
build:
	mkdir -p build/debug && \
	cd build/debug && \
	cmake $(GENERATOR) $(FORCE_COLOR) -DCMAKE_BUILD_TYPE=Debug ${BUILD_FLAGS} \
		../../duckdb/CMakeLists.txt -DEXTERNAL_EXTENSION_DIRECTORIES=../../duckdb-athena-extension -B. && \
	cmake --build . --config Debug -j


release:
	mkdir -p build/release && \
	cd build/release && \
	cmake $(GENERATOR) $(FORCE_COLOR) -DCMAKE_BUILD_TYPE=Release ${BUILD_FLAGS} \
		../../duckdb/CMakeLists.txt -DEXTERNAL_EXTENSION_DIRECTORIES=../../duckdb-athena-extension -B. && \
	cmake --build . --config Release


update:
	git submodule update --remote --merge