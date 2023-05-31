# This is an example build stage for the node template. Here we create the binary in a temporary image.

# This is a base image to build substrate nodes
FROM docker.io/paritytech/ci-linux:production as builder

ARG CARGO_FEATURES

WORKDIR /societal-node
RUN rustup default nightly-2023-01-01
RUN rustup target add wasm32-unknown-unknown --toolchain nightly-2023-01-01-x86_64-unknown-linux-gnu
COPY . .
RUN cargo build --locked --release --features=$CARGO_FEATURES

# This is the 2nd stage: a very small image where we copy the binary."
FROM docker.io/library/ubuntu:20.04
LABEL description="Multistage Docker image for Societal Node" \
  image.type="builder" \
  image.authors="Societal Labs <https://github.com/sctllabs>" \
  image.vendor="Societal Labs" \
  image.description="Multistage Docker image for Societal Node" \
  image.source="https://github.com/sctllabs/societal-node" \
  image.documentation="https://github.com/sctllabs/societal-node"

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates
RUN update-ca-certificates

# Copy the node binary.
COPY --from=builder /societal-node/target/release/societal-node /usr/local/bin

RUN useradd -m -u 1000 -U -s /bin/sh -d /node-dev node-dev && \
  mkdir -p /chain-data /node-dev/.local/share && \
  chown -R node-dev:node-dev /chain-data && \
  ln -s /chain-data /node-dev/.local/share/societal-node && \
  # check if executable works in this container
  /usr/local/bin/societal-node --version

USER node-dev

EXPOSE 30333 9933 9944 9615
VOLUME ["/chain-data"]

CMD ["/usr/local/bin/societal-node"]
