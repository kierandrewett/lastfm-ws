FROM rust:latest

WORKDIR /app

COPY . .

RUN cargo build --release

EXPOSE 8321

CMD ["./target/release/lastfm-ws"]