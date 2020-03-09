FROM rust:1-slim-buster as build

WORKDIR /usr/src/tarssh

# Make a blank project with our deps for Docker to cache.
# We skip rusty-sandbox because it does nothing useful on Linux.
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src \
  && echo 'fn main() { }' >src/main.rs \
  && cargo build --release --no-default-features --features drop_privs \
  && rm -r target/release/.fingerprint/tarssh-*

# Copy in the full project and build
COPY . .
RUN cargo build --release --no-default-features --features drop_privs

# Use a fairly minimal enviroment for deployment
FROM debian:buster-slim

RUN mkdir /var/empty && chmod 0555 /var/empty
COPY --from=build /usr/src/tarssh/target/release/tarssh /opt/tarssh

EXPOSE 22

ENTRYPOINT [ "/opt/tarssh" ]
CMD [ "-v", "--user=nobody", "--chroot=/var/empty", "--listen=0.0.0.0:22" ]
