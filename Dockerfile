# This is an example build stage for the node template. Here we create the binary in a temporary image.

# This is a base image to build substrate nodes
FROM docker.io/paritytech/ci-linux:production as builder

WORKDIR /societal-node
COPY . .
RUN cargo build --locked --release

# This is the 2nd stage: a very small image where we copy the binary."
FROM docker.io/library/ubuntu:20.04
LABEL description="Multistage Docker image for Societal Node" \
  image.type="builder" \
  image.authors="Societal Labs <https://github.com/sctllabs>" \
  image.vendor="Societal Labs" \
  image.description="Multistage Docker image for Societal Node" \
  image.source="https://github.com/sctllabs/societal-node" \
  image.documentation="https://github.com/sctllabs/societal-node"

# Copy the node binary.
COPY --from=builder /societal-node/target/release/societal-node /usr/local/bin

RUN useradd -m -u 1000 -U -s /bin/sh -d /node-dev node-dev && \
  mkdir -p /chain-data /node-dev/.local/share && \
  chown -R node-dev:node-dev /chain-data && \
  ln -s /chain-data /node-dev/.local/share/societal-node && \
  # unclutter and minimize the attack surface
  rm -rf /usr/bin /usr/sbin && \
  # check if executable works in this container
  /usr/local/bin/societal-node --version

USER node-dev

EXPOSE 30333 9933 9944 9615
VOLUME ["/chain-data"]

ENTRYPOINT ["/usr/local/bin/societal-node"]
