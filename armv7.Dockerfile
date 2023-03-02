FROM ghcr.io/cross-rs/armv7-unknown-linux-gnueabihf:main

RUN dpkg --add-architecture armhf
RUN apt-get update && apt-get install -y --no-install-recommends apt-utils libgsl-dev:armhf


ENV CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_RUSTFLAGS="-L /usr/lib/arm-linux-gnueabihf -C target-feature=+crt-static -C link-args=-Wl,-rpath-link,/usr/lib/arm-linux-gnueabihf $CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_RUSTFLAGS"
