FROM rust:slim-buster

RUN apt-get update && \
  apt-get -y upgrade && \
  apt-get -y install libpq-dev && \
  apt-get -y install pkg-config && \
  apt-get -y install libssl-dev

WORKDIR /app
COPY . /app/

RUN cargo build --release

EXPOSE 8080

ENTRYPOINT ["/bin/bash", "-c", "cargo run --release"]
