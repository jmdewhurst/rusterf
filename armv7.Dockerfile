ARG CROSS_BASE_IMAGE
FROM $CROSS_BASE_IMAGE

ARG CROSS_DEB_ARCH

RUN dpkg --add-architecture $CROSS_DEB_ARCH
RUN apt-get update && apt-get install -y --no-install-recommends apt-utils
# RUN apt-get update && apt-get -y install libgsl-dev:$CROSS_DEB_ARCH 
RUN apt-get update && apt-get -y install wget
RUN apt-get update && apt-get -y install gcc-arm-linux-gnueabihf

RUN wget "ftp://ftp.gnu.org/gnu/gsl/gsl-2.7.tar.gz"
RUN tar -zxvf gsl-2.7.tar.gz
RUN mkdir gsl_compiled
WORKDIR gsl-2.7
RUN ./configure --host=arm-linux-gnueabihf --prefix=/gsl_compiled
RUN make 
RUN make install

ENV CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_RUSTFLAGS="-C link-args=-Wl,-rpath-link,/usr/arm-linux-gnueabihf/lib -C target-feature=+crt-static $CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_RUSTFLAGS"
# ENV CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_RUSTFLAGS="-C link-args=-Wl,-rpath-link,-static -C target-feature=+crt-static $CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_RUSTFLAGS"

# RUN arm-linux-gnueabihf-gcc --print-file-name=libm.a
