# packet-gen

A procedural macro for generating zero-copy packet builders and parsers.

## Description

`packet-gen` lets you define packets, and automaticaly genereates code to build and parse them. The generated code acts as a typed view into a provided buffer without needing to copy, allocate, transfer ownership.

## Quick Start

Add `packet-gen` to your dependencies:

```toml
# Cargo.toml
[dependencies]
packet-gen = "0.1"
```

```rust
use packet_gen::packet;

// Define a packet with a type ID (0x05) and fields
packet! {pub MyPacket<BasePacket>(0x05) {
    a: uint8,
    b: uint16,
    c: uint32,
    d: float,
    e: [uint16; 16], // fixed-size array
    f: [uint8], // variable-length slice (must be last)
}}

// Define a packet with a response type for request/response pairs
packet! {pub ReadRequest<BasePacket>(0x01, ReadResponse) {
    field_id: uint8,
}}
packet! {pub ReadResponse<BasePacket>(0x01) {
    status: uint8,
	field_id: uint8,
    data: [uint8],
}}
```

## Generated API

For each packet, the macro generates:

### Building
- `try_init(buf)`: initialize a new packet in a provided buffer
- `set_<field>()`: mutator for each field
- `commit()`: finalize and return the total byte length

### Parsing
- `TryFrom<BasePacket<B>>`: convert a raw base packet into a typed packet
- `get_<field>()`: accessor for each field

### Response helpers
When a response type is specified, `try_alloc_response()` is generated to create a matching response packet.

## Field Types

| Syntax | Description |
|--------|-------------|
| `uint8` | 8-bit unsigned integer |
| `uint16` | 16-bit unsigned integer (little-endian) |
| `uint32` | 32-bit unsigned integer (little-endian) |
| `float` | 32-bit float (little-endian) |
| `[T; N]` | Fixed-size array of `N` elements |
| `[T]` | Variable-length slice (must be the last field) |
| `[{a: uint8, b: uint16}]` | Repeated struct (must be the last field) |

## The `BasePacket` Trait

The macro generates code that works with a `BasePacket<B>` type you define. Your `BasePacket` needs to implement:

- **Building** (`B: AsMut<[u8]>`): `try_init()`, `set_type()`, `set_id()`, `get_payload_mut()`, `commit()`
- **Parsing** (`B: AsRef<[u8]>`): `get_type()`, `get_id()`, `get_payload()`
