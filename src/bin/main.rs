#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use display_interface_spi::SPIInterfaceNoCS;
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use embedded_graphics_core::pixelcolor::Rgb565;
use embedded_graphics_core::prelude::DrawTarget;
use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_hal::gpio::{Level, Output, OutputConfig};
use esp_hal::spi::master::{Config as SpiConfig, Spi};
use esp_hal::time::Rate;
use esp_hal::timer::timg::TimerGroup;
use mipidsi::models::ILI9341Rgb565;
use mipidsi::options::{ColorInversion, Orientation};
use mipidsi::Builder;
use rtt_target::rprintln;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

extern crate alloc;

esp_bootloader_esp_idf::esp_app_desc!();

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
    let mut cs = Output::new(peripherals.GPIO10, Level::Low, OutputConfig::default());
    let _backlight = Output::new(peripherals.GPIO45, Level::High, OutputConfig::default());

    // We keep CS asserted for this single-display setup.
    cs.set_low();

    let di = SPIInterfaceNoCS::new(spi, dc);

    let mut display = Builder::new(ILI9341Rgb565, di)
        .display_size(240, 320)
        .orientation(Orientation::Landscape(false))
        .invert_colors(ColorInversion::Inverted)
        .init(&mut delay)
        .expect("Failed to initialize ILI9341 display");

    display
        .clear(Rgb565::RED)
        .expect("Failed to clear display to red");

    rprintln!("Display initialized and filled with solid red");

    loop {
        Timer::after(Duration::from_secs(1)).await;
    }
}
