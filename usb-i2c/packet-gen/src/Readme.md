# Packet Generation

The following is an example that creates a 

``` rust
packet!{MyPacket<BasePacket>(0x05) {
    a: uint8,
    b: uint16,
    c: uint32,
    d: float,
    e: [uint16; 16], // an array of 
    f: [{a: uint8, b: uint16}]
}};
```
