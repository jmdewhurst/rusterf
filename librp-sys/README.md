
# Partial rust bindings for the Red Pitaya api 

Bindings generated by bindgen --- see [this guide](https://medium.com/dwelo-r-d/using-c-libraries-in-rust-13961948c72a) and the [bindgen wiki](https://rust-lang.github.io/rust-bindgen/command-line-usage.html).

Generated `src/bindings.rs` using the `bindgen-cli` command 

    `bindgen include/rp.h -o src/bindings.rs --allowlist-file include/rp.h`

The C header file used by bindgen is located in `include/` --- note that it is NOT a direct copy of the official RP API header file found on [their github](https://github.com/RedPitaya/RedPitaya). There is a function I've added to `rp.h` which simply exposes an internal function in the API that allows for direct access to the RP oscilloscope acquisition buffer. The function is defined in `include/rp.c`. If you want to use these rust bindings, you will have to recompile the Red Pitaya API with these two modified files.

These bindings do NOT expose all of the functionality of the Red Pitaya API. I have only wrapped those functions which I expect to use in the operation of the scanning interferometer laser lock program.

## Structure of the Bindings 

These bindings expose four modules to the user:

1. `pitaya` is the 'base' module: instantiating a `pitaya::Pitaya` will load the Red Pitaya bitmap onto the FPGA and reset the FPGA state. This includes internal API operations like opening the memory map into the FPGA registers. When the `Pitaya` is dropped, it will release these resources. 

2. `oscilloscope` represents the onboard data acquisition module. Instantiating an `oscilloscope::Oscilloscope` will allow the user to make calls to oscilloscope settings and take data. Only one `Oscilloscope` should be valid at a time --- the bindings should prevent the creation of two different `Oscilloscope`s at the same time.

3. `generator` represents the onboard Arbitrary Waveform Generator. Again, instantiating this will allow the use of the waveform generator, and no two instances should be able to coexist.

4. `dpin` represents the digital IO pins on board, including the LEDs. Again, instantiating this will allow the use of the digital IO pins, and no two instances should be able to coexist.

