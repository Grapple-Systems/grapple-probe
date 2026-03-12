# Grapple Probe

An open source debug probe supporting the cmsis-dap interface.

## Building and Flashing

The firmware is written in rust so an installation of rust is required to build.  [Installation Instructions](https://doc.rust-lang.org/book/ch01-01-installation.html)

To build for the Grapple Probe hardware make sure you're current directory is this plaform folder, and build the cmsisdap bin.

``` bash
# install the thumbv6 target if not already installed
rustup target add thumbv6m-none-eabi

cargo build --release --bin cmsisdap
```

To flash the firmware to the Grapple Probe it first needs to be converted to UF2 format.

``` bash
# install elf2uf2 if not already installed
cargo install elf2uf2-rs
# convert the firmware to uf2 format
elf2uf2-rs target/thumbv6m-none-eabi/release/cmsisdap cmsisdap
```

A file named cmsisdap.uf2 should exist in the current directory.  Hold down the program button when plugging in the Grapple Probe to a computer.  The Grapple Probe will show up as a hard drive, mount the drive and copy over the uf2 file to upgrade the firmware.
