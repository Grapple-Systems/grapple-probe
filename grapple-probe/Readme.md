# Grapple Probe Library

The root crate is a std library for interacting with a grapple probe over USB.  The library can find and enumerate Grapple Probes, show status, and modify configuration values.

Some of the functionality is slit between crates in subdirectories described below:

- /cli: A command line interface for the functionality provided by the root crate.
- /protocol: A no_std library providing abstractions for the USB protocol and configuration fields.
