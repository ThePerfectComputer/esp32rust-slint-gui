# AGENTS.md

## Purpose
This project targets an ESP32-S3 board with an ILI9341 SPI display.
Use this file as the local runbook for future coding agents.

## Notes
- Use `cargo build` for validation.
- Do not flash from the agent by default.
- The user typically handles flashing manually.

## Environment
- Rust toolchain: `esp` (see `rust-toolchain.toml`)
- Before building in a fresh shell:
  - `source "$HOME/export-esp.sh"`

## Common Commands
- Build:
  - `source "$HOME/export-esp.sh" && cargo build`
- Check:
  - `source "$HOME/export-esp.sh" && cargo check`
- Run (only when explicitly requested):
  - Consider timeboxing `cargo run` to about 8 seconds, because probe/RTT sessions can stay attached indefinitely and block agent-side command completion.

## Current Display Bring-up Context
- Main entrypoint: `src/bin/main.rs`
- Display stack:
  - `esp-hal` SPI bus
  - `embedded-hal-bus::spi::ExclusiveDevice` for `SpiDevice`
  - `mipidsi` + `ILI9341Rgb565`

## Hardware Pin Mapping (LCDWiki board)
- https://www.lcdwiki.com/2.8inch_ESP32-S3_Display#Screen_parameters
- `IO10`: LCD CS
- `IO46`: LCD D/C
- `IO12`: LCD SCLK
- `IO11`: LCD MOSI
- `IO13`: LCD MISO
- `IO45`: LCD backlight
- Display reset is tied to board reset.

## Editing Guidance
- Keep changes small and testable.

## Commit Guidance
- Commit focused, incremental steps.
- Mention what changed on-device (e.g., “red shows cyan”, “checkerboard fills full panel”).
