FROM rust:1.31

WORKDIR /usr/src/tarssh
COPY . .

RUN cargo install --path .

CMD ["tarssh"]
