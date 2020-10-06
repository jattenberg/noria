FROM rust:latest as noriacontainer

WORKDIR /app

COPY . .

RUN apt-get update && apt-get install -y clang libclang-dev libssl-dev liblz4-dev build-essential

RUN cargo build --release --bin noria-server

CMD ["cargo", "r", "--release", "--bin", "noria-server", "--", "--deployment", "myapp", "--no-reuse", "--address", "172.16.0.19", "--shards", "0"]
