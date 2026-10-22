# Grapple Probe

A library for interfacing with the Grapple Probe.

## Features

- **`std`** (default) USB Probe enumeration, status queries, and configuration management.
- **`no_std` compatible** Disable the default `std` feature to use the crate in embedded/firmware environments.

## Usage

### USB Host (with `std`)

```rust
// Enumerate all connected Grapple Probes and print their status.
for probe in grapple_probe::GrappleProbe::all() {
    println!("grapple probe: {}", probe.get_serial_number());

    let mut dap = probe.open_cmsis_dap().unwrap();
    let status = dap.get_status().unwrap();
    println!("\ttvcc: {} mv, signal: {} mv", status.target_mv, status.signal_mv);
}
```

### Embedded (without `std`)

```toml
[dependencies]
grapple-probe = { version = "0.1", default-features = false }
```

```rust
// Parse protocol packets or manipulate configuration fields without std.
use grapple_probe::{proto, field, proto::ResetType};
```

## Modules

| Module | Description |
|---|---|
| `proto` | Protocol packet definitions (requests, responses, `ResetType`) |
| `field` | Configuration field definitions (`PowerControl`, etc.) |
| `host` *(std)* | USB device enumeration and communication |

## Error Types

- `proto::Error`: Protocol-level errors (`NotEnoughBytes`, `InvalidPacket`)
- `Error`: Host-side USB errors (`PermissionDenied`, `USBError`, `Timeout`, etc.)