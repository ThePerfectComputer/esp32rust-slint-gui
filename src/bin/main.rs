#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

extern crate alloc;

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use my_esp_project::embedded::display::{
    DISPLAY_HEIGHT, DISPLAY_WIDTH, DrawBuffer, clear_display,
};
use my_esp_project::embedded::touch::{
    TOUCH_I2C_KHZ, TOUCH_POLL_INTERVAL_MS, init_touch_controller, poll_and_dispatch_touch,
};
use my_esp_project::{DemoApp, install_demo_logic};
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
use slint::platform::software_renderer::Rgb565Pixel;
use slint::ComponentHandle;

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    rprintln!("PANIC: {:?}", info);
    loop {}
}

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(_spawner: Spawner) -> ! {
    rtt_target::rtt_init_print!();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    esp_alloc::heap_allocator!(size: 128 * 1024);
    rprintln!("Heap initialized ({} bytes free)", esp_alloc::HEAP.free());

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0);

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
    let touch_addr = init_touch_controller(&mut i2c);

    let spi_device = ExclusiveDevice::new_no_delay(spi, cs).expect("Failed to create SPI device");
    let mut spi_buffer = [0u8; 512];
    let di = SpiInterface::new(spi_device, dc, &mut spi_buffer);

    let mut display = Builder::new(ILI9341Rgb565, di)
        .display_size(240, 320)
        .orientation(Orientation::new().rotate(Rotation::Deg90))
        .color_order(ColorOrder::Bgr)
        .invert_colors(ColorInversion::Inverted)
        .init(&mut delay)
        .expect("Failed to initialize ILI9341 display");

    clear_display(&mut display).expect("Failed to clear display to black");

    let backend_state = my_esp_project::embedded::slint_platform::install();

    let app = DemoApp::new().expect("Failed to create Slint app");
    install_demo_logic(&app);
    app.show().expect("Failed to show Slint app");
    app.window().request_redraw();

    if let Some(window) = backend_state.window.borrow().as_ref() {
        window.set_size(slint::PhysicalSize::new(
            u32::from(DISPLAY_WIDTH),
            u32::from(DISPLAY_HEIGHT),
        ));
    }

    rprintln!("Slint touch demo active");

    let mut draw_buffer = [Rgb565Pixel(0); DISPLAY_WIDTH as usize];
    let mut renderer = DrawBuffer {
        display,
        buffer: &mut draw_buffer,
    };

    let mut consecutive_read_errors: u8 = 0;
    let mut last_touch_position: Option<slint::LogicalPosition> = None;

    loop {
        slint::platform::update_timers_and_animations();

        if let Some(window) = backend_state.window.borrow().as_ref().cloned() {
            poll_and_dispatch_touch(
                &mut i2c,
                touch_addr,
                &window,
                &mut last_touch_position,
                &mut consecutive_read_errors,
            );

            window.draw_if_needed(|software_renderer| {
                software_renderer.render_by_line(&mut renderer);
            });
        }

        Timer::after(Duration::from_millis(TOUCH_POLL_INTERVAL_MS)).await;
    }
}
