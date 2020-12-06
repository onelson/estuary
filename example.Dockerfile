# This Dockerfile serves as an example of how to run Estuary

FROM rust:slim-buster

# Estuary relies on being able to run `git` on the command-line.
# It additionally uses the `git2` crate which indirectly depends on `libssl`.
RUN apt-get update && apt-get install -y \
  git \
  pkg-config libssl-dev \
  && rm -rf /var/lib/apt/lists/*

RUN cargo install estuary --vers=0.1.0-alpha


# Use a volume to store our service data
VOLUME ["/var/lib/estuary"]

# Configure the service.
#
# These env vars will get the files Estuary needs to write into our volume and
# enable some basic logging HOWEVER you'll still need to configure the
# **base url** based on the public host/port you want to use.
ENV ESTUARY_INDEX_DIR="/var/lib/estuary/index" \
    ESTUARY_CRATE_DIR="/var/lib/estuary/crates" \
    RUST_LOG="actix_web=INFO,estuary=INFO"

EXPOSE 7878

# When running the container, don't forget you'll need to specify the base url
# either via a flag or environment variable.
ENTRYPOINT ["estuary"]
