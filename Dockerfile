FROM rust:latest as noriacontainer

WORKDIR /app

COPY . .

RUN apt-get update && apt-get install -y clang libclang-dev libssl-dev liblz4-dev build-essential

RUN cargo build --release --bin noria-server
