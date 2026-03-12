# Protocol

| Index | Description |
| ----- | ----------- |
| 0 | Command |
| 1 | Packet ID |
| 2 - N | Data |

## General Error ( command: 255 )

Response when the device doesn't understand a command.

| Index | Description |
| ----- | ----------- |
| 0 | 255 - general error |
| 1 | Packet ID ( copied from the second byte of the received packet if there is one, otherwise 0) |

## Configure Command ( command: 0 )

Configure the i2c bus.

Request:

| Index | Description |
| ----- | ----------- |
| 0 | 0 - configure command |
| 1 | Packet ID |
| 2 | Version: 0 |
| 3 - 7 | 32 bit unsigned integer of bus frequency in hz |

Response:

| Index | Description |
| ----- | ----------- |
| 0 | 0 - configure command |
| 1 | Packet ID |
| 2 | Result: 0 - success, 255 - invalid parameters |

## Write Command (command: 1)

Write some bytes to an i2c device with a stop at the end.

Request:

| Index | Description |
| ----- | ----------- |
| 0 | 1 - write command |
| 1 | Packet ID |
| 2 - 4 | 16 bit device address, msb is a 0 if a 7 bit address, or a 1 for a 10 bit address |
| 4 - N | Data to write to the i2c bus |

Response:

| Index | Description |
| ----- | ----------- |
| 0 | 1 - write command |
| 1 | Packet ID |
| 2 | Result: 0 - success, 1 - protocol error, 255 - invalid parameters |

## Read Command ( command: 2 )

Read some bytes from an i2c device with a stop at the end.

Request:

| Index | Description |
| ----- | ----------- |
| 0 | 2 - read command |
| 1 | Packet ID |
| 2 - 4 | 16 bit device address, msb is a 0 if a 7 bit address, or a 1 for a 10 bit address |
| 4 | Number of bytes to read from the i2c device |

Response:

| Index | Description |
| ----- | ----------- |
| 0 | 2 - read command |
| 1 | Packet ID |
| 2 | Result: 0 - success, 1 - protocol error, 255 - invalid parameters |
| 3 - N | Data read from the i2c device |

## Write-Read Command (command: 3)

Write some bytes from an i2c device then read some bytes from the same i2c device without a stop between the write and read.

Request:

| Index | Description |
| ----- | ----------- |
| 0 | 3 - write_read command |
| 1 - 3 | 16 bit device address, msb is a 0 if a 7 bit address, or a 1 for a 10 bit address |
| 3 | Number of bytes to read from the i2c device |
| 4 - N | Data to write to the i2c bus |

Response:

| Index | Description |
| ----- | ----------- |
| 0 | 3 - write_read command |
| 1 | Packet ID |
| 2 | Result: 0 - success, 1 - protocol error, 255 - invalid parameters |
| 3 - N | Data read from the i2c device |
