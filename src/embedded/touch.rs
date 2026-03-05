use alloc::rc::Rc;
use rtt_target::rprintln;
use slint::platform::software_renderer::MinimalSoftwareWindow;
use slint::platform::{PointerEventButton, WindowEvent};

use super::display::{DISPLAY_HEIGHT, DISPLAY_WIDTH};

const CST816_I2C_ADDR: u8 = 0x15;
const FT6X36_I2C_ADDR: u8 = 0x38;
const CST816_REG_CHIP_ID: u8 = 0xA7;
const FT6X36_REG_CHIP_ID: u8 = 0xA3;
const FT6X36_REG_THRESHHOLD: u8 = 0x80;
const FT6X36_TOUCH_THRESHOLD: u8 = 16;
const TOUCH_REG_GESTURE_ID: u8 = 0x01;

pub const TOUCH_I2C_KHZ: u32 = 400;
pub const TOUCH_POLL_INTERVAL_MS: u64 = 10;

fn map_touch_to_display(raw_x: u16, raw_y: u16) -> (u16, u16) {
    let x = raw_y.min(DISPLAY_WIDTH.saturating_sub(1));
    let y = raw_x.min(DISPLAY_HEIGHT.saturating_sub(1));
    (x, y)
}

fn parse_cst816_touch(data: &[u8; 6]) -> Option<(u16, u16, u8, u8)> {
    let gesture = data[0];
    let points = data[1] & 0x0F;

    if points == 0 {
        return None;
    }

    let x = ((u16::from(data[2] & 0x0F)) << 8) | u16::from(data[3]);
    let y = ((u16::from(data[4] & 0x0F)) << 8) | u16::from(data[5]);

    Some((x, y, points, gesture))
}

pub fn init_touch_controller<I2C>(i2c: &mut I2C) -> u8
where
    I2C: embedded_hal::i2c::I2c,
    I2C::Error: core::fmt::Debug,
{
    let mut chip_id = [0u8; 1];
    let cst816_ok = i2c
        .write_read(CST816_I2C_ADDR, &[CST816_REG_CHIP_ID], &mut chip_id)
        .is_ok();
    let ft6x36_ok = if cst816_ok {
        false
    } else {
        i2c.write_read(FT6X36_I2C_ADDR, &[FT6X36_REG_CHIP_ID], &mut chip_id)
            .is_ok()
    };

    let touch_addr = if cst816_ok {
        CST816_I2C_ADDR
    } else if ft6x36_ok {
        FT6X36_I2C_ADDR
    } else {
        rprintln!("No touch controller detected at 0x15 or 0x38; defaulting to 0x15");
        CST816_I2C_ADDR
    };

    let chip_id_reg = if touch_addr == CST816_I2C_ADDR {
        CST816_REG_CHIP_ID
    } else {
        FT6X36_REG_CHIP_ID
    };

    if let Err(err) = i2c.write_read(touch_addr, &[chip_id_reg], &mut chip_id) {
        rprintln!("Touch chip-id read failed at 0x{:02X}: {:?}", touch_addr, err);
    }

    rprintln!(
        "Touch controller: addr=0x{:02X}, chip-id=0x{:02X}",
        touch_addr,
        chip_id[0]
    );

    if touch_addr == FT6X36_I2C_ADDR {
        match i2c.write(
            touch_addr,
            &[FT6X36_REG_THRESHHOLD, FT6X36_TOUCH_THRESHOLD],
        ) {
            Ok(()) => {
                let mut threshold = [0u8; 1];
                match i2c.write_read(touch_addr, &[FT6X36_REG_THRESHHOLD], &mut threshold) {
                    Ok(()) => rprintln!(
                        "FT6x36 THRESHHOLD={} (readback={})",
                        FT6X36_TOUCH_THRESHOLD,
                        threshold[0]
                    ),
                    Err(err) => rprintln!(
                        "FT6x36 THRESHHOLD set to {}, readback failed: {:?}",
                        FT6X36_TOUCH_THRESHOLD,
                        err
                    ),
                }
            }
            Err(err) => rprintln!(
                "FT6x36 THRESHHOLD set failed ({}): {:?}",
                FT6X36_TOUCH_THRESHOLD,
                err
            ),
        }
    }

    touch_addr
}

fn release_pointer_if_pressed(
    window: &Rc<MinimalSoftwareWindow>,
    last_touch_position: &mut Option<slint::LogicalPosition>,
) {
    if let Some(pos) = last_touch_position.take() {
        let _ = window.try_dispatch_event(WindowEvent::PointerReleased {
            position: pos,
            button: PointerEventButton::Left,
        });
        let _ = window.try_dispatch_event(WindowEvent::PointerExited);
    }
}

pub fn poll_and_dispatch_touch<I2C>(
    i2c: &mut I2C,
    touch_addr: u8,
    window: &Rc<MinimalSoftwareWindow>,
    last_touch_position: &mut Option<slint::LogicalPosition>,
    consecutive_read_errors: &mut u8,
)
where
    I2C: embedded_hal::i2c::I2c,
    I2C::Error: core::fmt::Debug,
{
    let mut touch_data = [0u8; 6];

    match i2c.write_read(touch_addr, &[TOUCH_REG_GESTURE_ID], &mut touch_data) {
        Ok(()) => {
            *consecutive_read_errors = 0;
            if let Some((raw_x, raw_y, _points, _gesture)) = parse_cst816_touch(&touch_data) {
                let (x, y) = map_touch_to_display(raw_x, raw_y);
                let pos =
                    slint::PhysicalPosition::new(i32::from(x), i32::from(y)).to_logical(window.scale_factor());

                let event = if let Some(previous_pos) = last_touch_position.replace(pos) {
                    if previous_pos != pos {
                        Some(WindowEvent::PointerMoved { position: pos })
                    } else {
                        None
                    }
                } else {
                    Some(WindowEvent::PointerPressed {
                        position: pos,
                        button: PointerEventButton::Left,
                    })
                };

                if let Some(event) = event {
                    let _ = window.try_dispatch_event(event);
                }
            } else {
                release_pointer_if_pressed(window, last_touch_position);
            }
        }
        Err(err) => {
            *consecutive_read_errors = consecutive_read_errors.saturating_add(1);
            release_pointer_if_pressed(window, last_touch_position);
            if *consecutive_read_errors == 1 || *consecutive_read_errors % 8 == 0 {
                rprintln!(
                    "touch read error ({} consecutive): {:?}",
                    consecutive_read_errors,
                    err
                );
            }
        }
    }
}
