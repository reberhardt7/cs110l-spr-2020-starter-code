FROM ubuntu:18.04

RUN apt-get update && \
    apt-get install -y build-essential make curl strace gdb

# Install Rust. Don't use rustup, so we can install for all users (not just the
# root user)
RUN curl --proto '=https' --tlsv1.2 -sSf \
        https://static.rust-lang.org/dist/rust-1.43.0-x86_64-unknown-linux-gnu.tar.gz \
        -o rust.tar.gz && \
    tar -xzf rust.tar.gz && \
    rust-1.43.0-x86_64-unknown-linux-gnu/install.sh

# Make .cargo writable by any user (so we can run the container as an
# unprivileged user)
RUN mkdir /.cargo && chmod 777 /.cargo

WORKDIR /deet
