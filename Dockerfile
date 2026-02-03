FROM rust:latest

WORKDIR /usr/src/app

# copy only manifests first (cache dependencies)
COPY Cargo.toml Cargo.lock ./

RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -rf src

# copy actual source
COPY . .

RUN cargo build --release

EXPOSE 8080

CMD ["./target/release/skillvine"]
