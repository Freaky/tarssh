FROM rust:1.31

WORKDIR /usr/src/tarssh
COPY . .

RUN cargo install --path .
RUN mkdir /var/empty
RUN chown nobody:nogroup /var/empty
RUN chmod u=rx,g=rx,o-rwx /var/empty

CMD ["tarssh","-v"]
