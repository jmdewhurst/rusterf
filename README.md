
## Build 

Having installed all of the prerequisites, the build process should be as simple as running 
```
cross build --release --target=armv7-unknown-linux-gnueabihf
```
which will produce an executable `rusterf` at 
```
./target/armv7-unknown-linux-gnueabihf/release/rusterf
```
This executable can be copied to the Red Pitaya (e.g. via `scp`) as well as a `config.toml` file such as the one found at `./example/config.toml`.

If the build fails, try opening Docker Desktop and then running the command again.

## Build Prerequisites

Most of the build process is handled for us by a rust crate called Cross. The developers of Cross maintain a set of docker containers (which are like small virtual machines) for use in cross-compiling rust or C/C++ code. This means we don't need to fuss around with lots of cross-compilation toolchains or libraries. The things we need are 

### A native C++ toolchain

This is what the rust installation will use to install itself. This will depend on your system. For Windows, install the Visual Studio C++ Tools (where to find the downloads changes regularly, but should be fairly easily findable on google). On Linux distributions, install the appropriate build packages through your distribution's package manager. For instance, for Ubuntu users would enter
```
sudo apt install build-essential
```
in a terminal.

### Rust 

Install Rust via the rustup distribution: this will install the compiler as well as the build tool `cargo`. Specific instructions can be found on the rustup website. 

After installing rustup, you'll need to enter the following command into a terminal:
```
rustup target add armv7-unknown-linux-gnueabihf
```
This will give the rust compiler the ability to produce code for the Red Pitaya.

### Cross 

Having installed Rust via rustup, simply run 
```
cargo install cross
```
in a terminal. If you have installed Rust and a C++ toolchain, this should collect and install the utilities that will coordinate building software for the Red Pitaya. 

### Docker 

Docker is a "container engine," meaning it's in charge of running containers, which are like small lightweight virtual machines. They are useful so that an application can be bundled together with all of its dependencies; in our case, the compilation environment that the Cross developers maintain. Installation instructions can be found on the Docker website. Note that there are two pieces of software: the docker *engine* and *Docker Desktop*. We only need the engine, but it comes with Docker Desktop, so it's probably easiest to just install Docker Desktop.

**Note:** For Ubuntu users, there is a package called `docker`. **THIS IS NOT THE RIGHT PACKAGE**.

In order to run Docker applications, (i.e. in order to build code for the Red Pitaya) the docker engine needs to be running. This should be as simple as opening the Docker Desktop application.

