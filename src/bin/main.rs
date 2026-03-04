#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use embedded_graphics_core::draw_target::DrawTarget;
use embedded_graphics_core::pixelcolor::{Rgb565, RgbColor};
use embedded_hal_bus::spi::ExclusiveDevice;
use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_hal::gpio::{Level, Output, OutputConfig};
use esp_hal::i2c::master::{Config as I2cConfig, I2c};
use esp_hal::spi::master::{Config as SpiConfig, Spi};
use esp_hal::time::Rate;
use esp_hal::timer::timg::TimerGroup;
use mipidsi::Builder;
use mipidsi::interface::SpiInterface;
use mipidsi::models::ILI9341Rgb565;
use mipidsi::options::{ColorInversion, ColorOrder, Orientation, Rotation};
use rtt_target::rprintln;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

extern crate alloc;

esp_bootloader_esp_idf::esp_app_desc!();

const CST816_I2C_ADDR: u8 = 0x15;
const FT6X36_I2C_ADDR: u8 = 0x38;
const CST816_REG_CHIP_ID: u8 = 0xA7;
const FT6X36_REG_CHIP_ID: u8 = 0xA3;
const FT6X36_REG_THRESHHOLD: u8 = 0x80;
const FT6X36_TOUCH_THRESHOLD: u8 = 16;
const TOUCH_REG_GESTURE_ID: u8 = 0x01;
const DISPLAY_WIDTH: u16 = 320;
const DISPLAY_HEIGHT: u16 = 240;
const TOUCH_I2C_KHZ: u32 = 400;
const TOUCH_POLL_INTERVAL_MS: u64 = 10;
const BRUSH_RADIUS: i16 = 1;

fn map_touch_to_display(raw_x: u16, raw_y: u16) -> (u16, u16) {
    // Display is rotated to landscape (Deg90), so map raw portrait touch data accordingly.
    let x = raw_y.min(DISPLAY_WIDTH.saturating_sub(1));
    let y = raw_x.min(DISPLAY_HEIGHT.saturating_sub(1));
    (x, y)
}

fn draw_line<F>(x0: u16, y0: u16, x1: u16, y1: u16, mut plot: F)
where
    F: FnMut(u16, u16),
{
    let mut x0 = i32::from(x0);
    let mut y0 = i32::from(y0);
    let x1 = i32::from(x1);
    let y1 = i32::from(y1);

    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        plot(x0 as u16, y0 as u16);
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

fn draw_brush<F>(x: u16, y: u16, radius: i16, mut plot: F)
where
    F: FnMut(u16, u16),
{
    let cx = i32::from(x);
    let cy = i32::from(y);
    let radius = i32::from(radius);

    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let nx = cx + dx;
            let ny = cy + dy;
            if nx >= 0 && nx < i32::from(DISPLAY_WIDTH) && ny >= 0 && ny < i32::from(DISPLAY_HEIGHT) {
                plot(nx as u16, ny as u16);
            }
        }
    }
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

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    rtt_target::rtt_init_print!();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0);

    let _ = spawner;

    let mut delay = Delay::new();

    let lcd_sclk = peripherals.GPIO12;
    let lcd_mosi = peripherals.GPIO11;
    let lcd_miso = peripherals.GPIO13;

    let spi = Spi::new(
        peripherals.SPI2,
        SpiConfig::default().with_frequency(Rate::from_mhz(40)),
    )
    .expect("Failed to initialize SPI2")
    .with_sck(lcd_sclk)
    .with_mosi(lcd_mosi)
    .with_miso(lcd_miso);

    let dc = Output::new(peripherals.GPIO46, Level::Low, OutputConfig::default());
    let cs = Output::new(peripherals.GPIO10, Level::High, OutputConfig::default());
    let _backlight = Output::new(peripherals.GPIO45, Level::High, OutputConfig::default());

    let mut touch_reset = Output::new(peripherals.GPIO18, Level::High, OutputConfig::default());
    let mut i2c = I2c::new(
        peripherals.I2C0,
        I2cConfig::default().with_frequency(Rate::from_khz(TOUCH_I2C_KHZ)),
    )
    .expect("Failed to initialize I2C0")
    .with_sda(peripherals.GPIO16)
    .with_scl(peripherals.GPIO15);

    touch_reset.set_low();
    delay.delay_millis(10);
    touch_reset.set_high();
    delay.delay_millis(50);

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

    if touch_addr == FT6X36_I2C_ADDR {
        match i2c.write(
            touch_addr,
            &[FT6X36_REG_THRESHHOLD, FT6X36_TOUCH_THRESHOLD],
        ) {
            Ok(()) => {
                let mut threshold = [0u8; 1];
                match i2c.write_read(touch_addr, &[FT6X36_REG_THRESHHOLD], &mut threshold) {
                    Ok(()) => rprintln!(
                        "FT6x36 sensitivity: THRESHHOLD set to {} (readback={})",
                        FT6X36_TOUCH_THRESHOLD,
                        threshold[0]
                    ),
                    Err(err) => rprintln!(
                        "FT6x36 sensitivity: set THRESHHOLD={}, readback failed: {:?}",
                        FT6X36_TOUCH_THRESHOLD,
                        err
                    ),
                }
            }
            Err(err) => rprintln!(
                "FT6x36 sensitivity: failed to set THRESHHOLD={} at reg 0x80: {:?}",
                FT6X36_TOUCH_THRESHOLD,
                err
            ),
        }
    }

    let spi_device = ExclusiveDevice::new_no_delay(spi, cs).expect("Failed to create SPI device");
    let mut buffer = [0u8; 512];
    let di = SpiInterface::new(spi_device, dc, &mut buffer);

    let mut display = Builder::new(ILI9341Rgb565, di)
        .display_size(240, 320)
        .orientation(Orientation::new().rotate(Rotation::Deg90))
        .color_order(ColorOrder::Bgr)
        .invert_colors(ColorInversion::Inverted)
        .init(&mut delay)
        .expect("Failed to initialize ILI9341 display");

    display
        .clear(Rgb565::BLACK)
        .expect("Failed to clear display to black");

    rprintln!("Display initialized; paint mode active (white draw on black)");

    let mut consecutive_read_errors: u8 = 0;
    let mut last_touch_point: Option<(u16, u16)> = None;

    loop {
        let mut touch_data = [0u8; 6];

        match i2c.write_read(touch_addr, &[TOUCH_REG_GESTURE_ID], &mut touch_data) {
            Ok(()) => {
                consecutive_read_errors = 0;
                if let Some((raw_x, raw_y, _points, _gesture)) = parse_cst816_touch(&touch_data) {
                    let (x, y) = map_touch_to_display(raw_x, raw_y);
                    let mut draw_failed = false;

                    if let Some((prev_x, prev_y)) = last_touch_point {
                        draw_line(prev_x, prev_y, x, y, |px, py| {
                            draw_brush(px, py, BRUSH_RADIUS, |bx, by| {
                                if display.set_pixel(bx, by, Rgb565::WHITE).is_err() {
                                    draw_failed = true;
                                }
                            });
                        });
                    } else {
                        draw_brush(x, y, BRUSH_RADIUS, |bx, by| {
                            if display.set_pixel(bx, by, Rgb565::WHITE).is_err() {
                                draw_failed = true;
                            }
                        });
                    }

                    if draw_failed {
                        rprintln!("display write failed while drawing touch trail");
                    }

                    last_touch_point = Some((x, y));

                } else {
                    last_touch_point = None;
                }
            }
            Err(err) => {
                consecutive_read_errors = consecutive_read_errors.saturating_add(1);
                last_touch_point = None;
                if consecutive_read_errors == 1 || consecutive_read_errors % 8 == 0 {
                    rprintln!(
                        "touch read error ({} consecutive): {:?}",
                        consecutive_read_errors,
                        err
                    );
                }
            }
        }

        Timer::after(Duration::from_millis(TOUCH_POLL_INTERVAL_MS)).await;
    }
}
