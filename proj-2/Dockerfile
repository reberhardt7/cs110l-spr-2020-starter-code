FROM ubuntu:18.04

RUN apt-get update && \
    apt-get install -y build-essential python3 curl

RUN useradd -ms /bin/bash balancebeam
USER balancebeam
WORKDIR /home/balancebeam

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y

COPY balancebeam/Cargo.toml .
RUN mkdir src && touch src/main.rs && ./.cargo/bin/cargo build --release || true

COPY balancebeam/ ./
RUN ./.cargo/bin/cargo build --release

ENTRYPOINT ["./.cargo/bin/cargo", "run", "--release", "--"]
