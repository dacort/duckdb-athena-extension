FROM ubuntu:latest

RUN apt-get update -y && \
    apt-get install -y \
        build-essential \
        cmake \
        curl \
        git \
        libclang-dev \
        ninja-build
RUN curl --proto '=https' --tlsv1.3 https://sh.rustup.rs -sSf | bash -s -- -y
RUN echo 'source $HOME/.cargo/env' >> $HOME/.bashrc


WORKDIR /app/duckdb-athena-extension
