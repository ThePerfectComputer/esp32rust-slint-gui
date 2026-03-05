use rtt_target::rprintln;

use super::display::{DISPLAY_HEIGHT, DISPLAY_WIDTH};

const FT6336_I2C_ADDR: u8 = 0x38;
const FT6336_REG_CHIP_ID: u8 = 0xA3;
const FT6336_REG_THRESHHOLD: u8 = 0x80;
const FT6336_TOUCH_THRESHOLD: u8 = 12;
const TOUCH_REG_GESTURE_ID: u8 = 0x01;

pub const TOUCH_I2C_KHZ: u32 = 400;
pub const TOUCH_POLL_INTERVAL_MS: u64 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TouchSample {
    Pressed { x: u16, y: u16 },
    Released,
    ReadError,
}

fn map_touch_to_display(raw_x: u16, raw_y: u16) -> (u16, u16) {
    // Display is configured with Rotation::Deg270 in ui_runtime.rs.
    // For this panel orientation, swap axes and mirror touch about the Y-axis.
    let x = raw_y.min(DISPLAY_WIDTH.saturating_sub(1));
    let y = DISPLAY_HEIGHT
        .saturating_sub(1)
        .saturating_sub(raw_x.min(DISPLAY_HEIGHT.saturating_sub(1)));
    (x, y)
}

fn parse_ft6336_touch(data: &[u8; 6]) -> Option<(u16, u16, u8, u8)> {
    let gesture = data[0];
    let points = data[1] & 0x0F;

    if points == 0 {
        return None;
    }

    let x = ((u16::from(data[2] & 0x0F)) << 8) | u16::from(data[3]);
    let y = ((u16::from(data[4] & 0x0F)) << 8) | u16::from(data[5]);

    Some((x, y, points, gesture))
}

pub fn init_touch_controller<I2C>(i2c: &mut I2C)
where
    I2C: embedded_hal::i2c::I2c,
    I2C::Error: core::fmt::Debug,
{
    let mut chip_id = [0u8; 1];
    if let Err(err) = i2c.write_read(FT6336_I2C_ADDR, &[FT6336_REG_CHIP_ID], &mut chip_id) {
        rprintln!(
            "Touch chip-id read failed at 0x{:02X}: {:?}",
            FT6336_I2C_ADDR,
            err
        );
        return;
    }

    rprintln!(
        "Touch controller: addr=0x{:02X}, chip-id=0x{:02X}",
        FT6336_I2C_ADDR,
        chip_id[0]
    );

    // seems to reduce aggressive filter and make touchscreen a little
    // more responsive
    match i2c.write(
        FT6336_I2C_ADDR,
        &[FT6336_REG_THRESHHOLD, FT6336_TOUCH_THRESHOLD],
    ) {
        Ok(()) => {
            let mut threshold = [0u8; 1];
            match i2c.write_read(FT6336_I2C_ADDR, &[FT6336_REG_THRESHHOLD], &mut threshold) {
                Ok(()) => rprintln!(
                    "FT6336 THRESHHOLD={} (readback={})",
                    FT6336_TOUCH_THRESHOLD,
                    threshold[0]
                ),
                Err(err) => rprintln!(
                    "FT6336 THRESHHOLD set to {}, readback failed: {:?}",
                    FT6336_TOUCH_THRESHOLD,
                    err
                ),
            }
        }
        Err(err) => rprintln!(
            "FT6336 THRESHHOLD set failed ({}): {:?}",
            FT6336_TOUCH_THRESHOLD,
            err
        ),
    }
}

pub fn poll_touch_sample<I2C>(i2c: &mut I2C, consecutive_read_errors: &mut u8) -> TouchSample
where
    I2C: embedded_hal::i2c::I2c,
    I2C::Error: core::fmt::Debug,
{
    let mut touch_data = [0u8; 6];

    match i2c.write_read(FT6336_I2C_ADDR, &[TOUCH_REG_GESTURE_ID], &mut touch_data) {
        Ok(()) => {
            *consecutive_read_errors = 0;
            if let Some((raw_x, raw_y, _points, _gesture)) = parse_ft6336_touch(&touch_data) {
                let (x, y) = map_touch_to_display(raw_x, raw_y);
                TouchSample::Pressed { x, y }
            } else {
                TouchSample::Released
            }
        }
        Err(err) => {
            *consecutive_read_errors = consecutive_read_errors.saturating_add(1);
            if *consecutive_read_errors == 1 || *consecutive_read_errors % 8 == 0 {
                rprintln!(
                    "touch read error ({} consecutive): {:?}",
                    consecutive_read_errors,
                    err
                );
            }
            TouchSample::ReadError
        }
    }
}
