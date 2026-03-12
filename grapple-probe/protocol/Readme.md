# Grapple Probe Protocol Library

A no_std library that abstracts the Grapple Probe USB protocol and configuration fields.

> [!NOTE]
> Packet and field definitions are made up of primitives which are all encoded in little endian byte order.

## Protocol

Grapple Probes use the CMSIS-DAP protocol to provide debug probe functionality over USB.  CMSIS-DAP sort of gives room for vendor specific commands because it doesn't use all of the command byte values for its own commands.  The Grapple Probe firmware extends the CMSIS-DAP command set by adding a new command `0xE0` for Grapple Probe commands.

### Get Status

Get the current status of the Grapple Probe:

- State of the ground detect pin.
- Measured voltage on the TVCC pin.
- Voltage commanded to the voltage translators.

**Request:**

| Index | Type | Description |
| ----- | ---- | ----------- |
| 0 | byte | 0xE0 - CMSIS Dap Id for Grapple packets |
| 1 | byte | 0x00 - Get Status Command |

**Response:**

| Index | Type | Description |
| ----- | ---- | ----------- |
| 0 | byte | 0xE0 - CMSIS Dap Id for Grapple packets |
| 1 | byte | 0x00 - Get Status Command |
| 2 | byte | Flags: 0x80 - Gnd Detect state |
| 3-4 | uint16 | Measured TVCC voltage in mv |
| 5-6 | uint16 | Commanded translator voltage in mv |

### Read Field

Read a configuration field from the Grapple Probe.

**Request:**

| Index | Type | Description |
| ----- | ---- | ----------- |
| 0 | byte | 0xE0 - CMSIS Dap Id for Grapple packets |
| 1 | byte | 0x01 - Read Command |
| 2 | uint8 | id of the field to read |

**Response:**

| Index | Type | Description |
| ----- | ---- | ----------- |
| 0 | byte | 0xE0 - CMSIS Dap Id for Grapple packets |
| 1 | byte | 0x01 - Read Command |
| 2 | byte | Status of request: 0x00 - Succeeded, 0xFF - Failed |
| 3 | uint8 | field id |
| 4.. | bytes | field data |

### Write Field

Write a configuration field to the grapple probe.

**Request:**

| Index | Type | Description |
| ----- | ---- | ----------- |
| 0 | byte | 0xE0 - CMSIS Dap Id for Grapple packets |
| 1 | byte | 0x02 - Write Command |
| 2 | uint8 | field id |
| 3.. | bytes | field data |

**Response:**

| Index | Type | Description |
| ----- | ---- | ----------- |
| 0 | byte | 0xE0 - CMSIS Dap Id for Grapple packets |
| 1 | byte | 0x02 - Write Command |
| 2 | byte | Status of request: 0x00 - Succeeded, 0xFF - Failed |
| 3 | uint8 | field id |

## Configuration Fields

Grapple Probes have some configuration ability through the read and write configuration fields commands.  Fields have a one byte id and a variable length string of bytes containing the data.  Each field has an independently defined structure for the data bytes.

### Power Control (id=0)

| Index | Type | Description |
| ----- | ---- | ----------- |
| 0 | byte | 0x00 - Version |
| 1 | byte | Flags |
| 2-3 | uint16 | Default signal voltage in mv |
| 4-5 | uint16 | TVCC voltage when an output in mv |

The Flags byte is formatted as follows:

| Bit Index | Description |
| --------- | ----------- |
| 7 | Set if the 5V Key output is enabled |
| 6 | Set if signal voltage should follow TVCC voltage |
| 5 | Set if signal voltage should be gated by Gnd Detect |
| 4 | Set if signal voltage should be gated by TVCC |
| 3 | Set if TVCC is an ouptut |
| 2 | Set if TVCC output should be gated by Gnd Detect |
| 1-0 | Reserved |