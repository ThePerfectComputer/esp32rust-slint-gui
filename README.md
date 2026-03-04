# About
This repository is for bringing up an **ESP32-S3 + 2.8" ILI9341 TFT + CST816 capacitive touch** board,
with the long-term goal of running Slint on top of embassy/esp-rs.

**Currently targeted device:** `2.8inch ESP32-S3 Display` family documented on LCDWiki
(the same class of board as the one linked below).

- Example board listing: https://www.amazon.com/dp/B0FKG7WRWV?ref=fed_asin_title
- Board docs + pinout: https://www.lcdwiki.com/2.8inch_ESP32-S3_Display

## Status
- [x] `cargo run` prints hello-world over RTT from board
- [x] Initialize ILI9341 over SPI and clear display to solid red
- [x] Initialize ILI9341 over SPI and clear display to show checkerboard pattern
- [x] Acquire touch coordinates
- [x] Implement touchscreen paint mode (black background, draw white points while dragging on the touchscreen)
- [ ] Slint rendering to display(should probably use 
      buffers at this points instead of doing `set_pixel`
      for each pixel)

## Display/Touch pin mapping (from LCDWiki)
Based on the LCDWiki ESP32 pin table for the 2.8" ESP32-S3 display board:

### LCD (ILI9341, SPI)
- `IO10`: LCD CS
- `IO46`: LCD D/C
- `IO12`: LCD SCLK
- `IO11`: LCD MOSI
- `IO13`: LCD MISO
- `RST`: shared with ESP32-S3 reset
- `IO45`: LCD backlight enable

### Touch (CST816, I2C)
- `IO16`: I2C SDA
- `IO15`: I2C SCL
- `IO18`: touch reset
- `IO17`: touch interrupt

Reference: https://www.lcdwiki.com/2.8inch_ESP32-S3_Display#ESP32_Pin_Parameters

## Getting started
### 1) Install esp-rs toolchain + utilities
This project expects the `esp` Rust toolchain and Xtensa target support.
A common setup flow is:

```bash
# install espup (once)
cargo install espup

# install/update esp toolchain + targets
espup install

# load exported environment variables for current shell
# (espup prints the exact source command/path to use)
source "$HOME/export-esp.sh"
```

You will also want a flashing/debug tool:

```bash
cargo install probe-rs-tools --locked
```

### 2) Build/check
```bash
cargo check
```

### 3) Run on hardware
With the board connected over USB and probe-rs configured for ESP32-S3:

```bash
cargo run
```

## Notes on the current display bring-up
The display path uses:
- `esp-hal` for SPI + GPIO
- `mipidsi` for ILI9341 initialization
- `display-interface-spi` to bridge SPI + D/C to the display interface

The current bring-up starts in **paint mode** (black background with white brush strokes from touch input).

## Possibly useful references
- https://www.lcdwiki.com/2.8inch_ESP32-S3_Display#Screen_parameters
- https://www.lcdwiki.com/2.8inch_ESP32-S3_Display#ESP32_Pin_Parameters
- https://github.com/slint-ui/slint/blob/master/examples/mcu-board-support/esp32_s3_box_3.rs
