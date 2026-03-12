# Grapple Probe

Firmware and software tools for the [Grapple Probe](https://grapple.systems/grapple-probe).

To get the project check it out with git.

``` sh
git clone --recursive https://github.com/Grapple-Systems/grapple-probe.git
```

To compile the Grapple Probe firmware see the [grapple-probe-firmware readme](grapple-probe-firmware/Readme.md)

## Directory Structure

- grapple-probe-firmware: The firmware for the Grapple Probe device.
- cmsis-dap: A library for cmsis-dap probes written in rust for the Embassy framework.
- grapple-probe: A library and host tools for interacting with the proprietary parts of the Grapple Probe.
- usb-i2c: A library and tools for an I2C port over usb.
- device-info: A library for packing device specific information into memory.
- packet-gen: Binary packet generation library using macros.
- test: Tools for testing and benchmarking parts of the Grapple Probe.

## Licenses

All the code in this repository is licensed either as GPL-3.0+ or MIT and Apache-2.0.  The bulk of the code for the grapple probe, grapple-probe and cmsis-dap subdirectories are licensed under GPL-3.0+.

## Contributing

Contributions are welcome, but must be released under the applicable license without any additional terms or conditions.
