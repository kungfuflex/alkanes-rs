# Build stage - compile Bitcoin Core from patched source
FROM debian:trixie-slim AS build

LABEL maintainer="Alkanes Team"

RUN apt-get update --yes \
  && apt-get install --yes --no-install-recommends \
    build-essential \
    ca-certificates \
    capnproto \
    ccache \
    clang-19 \
    cmake \
    git \
    libboost-dev \
    libcapnp-dev \
    libevent-dev \
    libsqlite3-dev \
    libzmq3-dev \
    pkg-config \
    python3 \
    systemtap-sdt-dev \
  && apt-get clean \
  && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

ENV BITCOIN_PREFIX=/opt/bitcoin

# Copy patched Bitcoin Core source (context is project root)
COPY patch/bitcoin /src/bitcoin
WORKDIR /src/bitcoin

# Build Bitcoin Core with CMake
RUN set -ex \
  && cmake -B build \
    -DBUILD_TESTS=OFF \
    -DBUILD_TX=ON \
    -DBUILD_UTIL=OFF \
    -DCMAKE_BUILD_TYPE=RelWithDebInfo \
    -DCMAKE_CXX_COMPILER=clang++-19 \
    -DCMAKE_C_COMPILER=clang-19 \
    -DCMAKE_INSTALL_PREFIX:PATH="${BITCOIN_PREFIX}" \
    -DWITH_CCACHE=ON \
  && cmake --build build -j$(nproc) \
  && strip build/bin/bitcoin-cli build/bin/bitcoin-tx build/bin/bitcoind \
  && cmake --install build

# Runtime stage - minimal image with compiled binaries
FROM debian:trixie-slim

ARG UID=101
ARG GID=101

ENV BITCOIN_DATA=/home/bitcoin/.bitcoin

RUN groupadd --gid ${GID} bitcoin \
  && useradd --create-home --no-log-init -u ${UID} -g ${GID} bitcoin \
  && apt-get update --yes \
  && apt-get install --yes --no-install-recommends \
    capnproto \
    gosu \
    libboost-dev \
    libevent-dev \
    libsqlite3-dev \
    libzmq3-dev \
    systemtap-sdt-dev \
  && apt-get clean \
  && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

COPY --from=build /opt/bitcoin /opt
ENV PATH=/opt/bin:$PATH

COPY docker/bitcoind/docker-entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

VOLUME ["/home/bitcoin/.bitcoin"]
EXPOSE 8332 8333 18332 18333 18443 18444 38333 38332

ENTRYPOINT ["/entrypoint.sh"]
RUN bitcoind -version | grep "Bitcoin Core"
CMD ["bitcoind"]
