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
use embedded_graphics_core::pixelcolor::{Rgb565, RgbColor};
use embedded_hal_bus::spi::ExclusiveDevice;
use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_hal::gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull};
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
const TOUCH_REG_GESTURE_ID: u8 = 0x01;

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
    let touch_irq = Input::new(
        peripherals.GPIO17,
        InputConfig::default().with_pull(Pull::Up),
    );

    let mut i2c = I2c::new(
        peripherals.I2C0,
        I2cConfig::default().with_frequency(Rate::from_khz(100)),
    )
    .expect("Failed to initialize I2C0")
    .with_sda(peripherals.GPIO16)
    .with_scl(peripherals.GPIO15);

    touch_reset.set_low();
    delay.delay_millis(10);
    touch_reset.set_high();
    delay.delay_millis(50);

    let mut found_any_i2c_device = false;
    let mut has_cst816_addr = false;
    let mut has_ft6x36_addr = false;
    let mut probe_buf = [0u8; 1];
    for addr in 0x08..=0x77 {
        if i2c.read(addr, &mut probe_buf).is_ok() {
            found_any_i2c_device = true;
            if addr == CST816_I2C_ADDR {
                has_cst816_addr = true;
            }
            if addr == FT6X36_I2C_ADDR {
                has_ft6x36_addr = true;
            }
            rprintln!("I2C device responded at 0x{:02X}", addr);
        }
    }
    if !found_any_i2c_device {
        rprintln!("I2C scan found no devices on I2C0 (SDA=GPIO16, SCL=GPIO15)");
    }

    let touch_addr = if has_cst816_addr {
        CST816_I2C_ADDR
    } else if has_ft6x36_addr {
        FT6X36_I2C_ADDR
    } else {
        CST816_I2C_ADDR
    };

    rprintln!("Selected touch I2C address: 0x{:02X}", touch_addr);

    let chip_id_reg = if touch_addr == CST816_I2C_ADDR {
        CST816_REG_CHIP_ID
    } else {
        FT6X36_REG_CHIP_ID
    };
    let mut chip_id = [0u8; 1];
    match i2c.write_read(touch_addr, &[chip_id_reg], &mut chip_id) {
        Ok(()) => rprintln!("Touch controller chip id: 0x{:02X}", chip_id[0]),
        Err(err) => rprintln!("Touch chip-id read failed: {:?}", err),
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

    const WIDTH: u16 = 320;
    const HEIGHT: u16 = 240;
    const TILE: u16 = 20;

    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let tile_x = x / TILE;
            let tile_y = y / TILE;
            let color = if (tile_x + tile_y) % 2 == 0 {
                Rgb565::RED
            } else {
                Rgb565::BLUE
            };

            display
                .set_pixel(x, y, color)
                .expect("Failed to draw checkerboard pixel");
        }
    }

    rprintln!("Display initialized and checkerboard drawn; polling touch at 8 Hz");

    let mut consecutive_read_errors: u8 = 0;

    loop {
        let irq_active = touch_irq.is_low();
        let mut touch_data = [0u8; 6];

        match i2c.write_read(touch_addr, &[TOUCH_REG_GESTURE_ID], &mut touch_data) {
            Ok(()) => {
                consecutive_read_errors = 0;
                if let Some((y, x, points, gesture)) = parse_cst816_touch(&touch_data) {
                    rprintln!(
                        "touch: x={} y={} points={} gesture=0x{:02X} irq={}",
                        x,
                        y,
                        points,
                        gesture,
                        irq_active
                    );
                } else if irq_active {
                    rprintln!("touch irq active but no points decoded");
                }
            }
            Err(err) => {
                consecutive_read_errors = consecutive_read_errors.saturating_add(1);
                if consecutive_read_errors == 1 || consecutive_read_errors % 8 == 0 {
                    rprintln!(
                        "touch read error ({} consecutive): {:?}",
                        consecutive_read_errors,
                        err
                    );
                }
            }
        }

        Timer::after(Duration::from_millis(125)).await;
    }
}
