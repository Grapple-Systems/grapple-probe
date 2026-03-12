# Device Info

A library for storing and generating device info for storage on an EEPROM or Flash.

| Index | Description |
| ----- | ----------- |
| 0 - 1 | Checksum of all bytes after checksum, fletcher-16 |
| 2 - 3 | Fields size in bytes (u16) |
| 4+ | Fields |

Field

| Index | Description |
| ----- | ----------- |
| 0 - 1 | id (u16) |
| 2 - 3 | Payload size in bytes (u16) |
| 4+ | Field payload |

## Device ID (0)

A string that is a unique id for the device.
