use alloc::rc::Rc;

use embassy_time::{Duration, Timer};
use embedded_hal_bus::spi::ExclusiveDevice;
use esp_hal::delay::Delay;
use esp_hal::gpio::{AnyPin, Level, Output, OutputConfig};
use esp_hal::i2c::master::{Config as I2cConfig, I2c};
use esp_hal::spi::master::{Config as SpiConfig, Spi};
use esp_hal::time::Rate;
use mipidsi::Builder;
use mipidsi::interface::SpiInterface;
use mipidsi::models::ILI9341Rgb565;
use mipidsi::options::{ColorInversion, ColorOrder, Orientation, Rotation};
use rtt_target::rprintln;
use slint::ComponentHandle;
use slint::platform::software_renderer::{MinimalSoftwareWindow, Rgb565Pixel};
use slint::platform::{PointerEventButton, WindowEvent};

use crate::embedded::display::{DISPLAY_HEIGHT, DISPLAY_WIDTH, DrawBuffer, clear_display};
use crate::embedded::slint_backend::install_platform;
use crate::embedded::touch::{
    TOUCH_I2C_KHZ, TOUCH_POLL_INTERVAL_MS, TouchSample, init_touch_controller, poll_touch_sample,
};
use crate::{DemoApp, install_demo_logic};

pub struct UiTaskResources {
    pub spi2: esp_hal::peripherals::SPI2<'static>,
    pub i2c0: esp_hal::peripherals::I2C0<'static>,
    pub lcd_sclk: AnyPin<'static>,
    pub lcd_mosi: AnyPin<'static>,
    pub lcd_miso: AnyPin<'static>,
    pub lcd_dc: AnyPin<'static>,
    pub lcd_cs: AnyPin<'static>,
    pub lcd_backlight: AnyPin<'static>,
    pub touch_reset: AnyPin<'static>,
    pub touch_sda: AnyPin<'static>,
    pub touch_scl: AnyPin<'static>,
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

fn dispatch_touch_to_slint(
    window: &Rc<MinimalSoftwareWindow>,
    sample: TouchSample,
    last_touch_position: &mut Option<slint::LogicalPosition>,
) {
    let TouchSample::Pressed { x, y } = sample else {
        release_pointer_if_pressed(window, last_touch_position);
        return;
    };

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
}

#[embassy_executor::task]
pub async fn run_ui(resources: UiTaskResources) -> ! {
    let UiTaskResources {
        spi2,
        i2c0,
        lcd_sclk,
        lcd_mosi,
        lcd_miso,
        lcd_dc,
        lcd_cs,
        lcd_backlight,
        touch_reset,
        touch_sda,
        touch_scl,
    } = resources;

    let mut delay = Delay::new();

    let spi = Spi::new(
        spi2,
        SpiConfig::default().with_frequency(Rate::from_mhz(40)),
    )
    .expect("Failed to initialize SPI2")
    .with_sck(lcd_sclk)
    .with_mosi(lcd_mosi)
    .with_miso(lcd_miso);

    let dc = Output::new(lcd_dc, Level::Low, OutputConfig::default());
    let cs = Output::new(lcd_cs, Level::High, OutputConfig::default());
    let _backlight = Output::new(lcd_backlight, Level::High, OutputConfig::default());

    let mut touch_reset = Output::new(touch_reset, Level::High, OutputConfig::default());
    let mut i2c = I2c::new(
        i2c0,
        I2cConfig::default().with_frequency(Rate::from_khz(TOUCH_I2C_KHZ)),
    )
    .expect("Failed to initialize I2C0")
    .with_sda(touch_sda)
    .with_scl(touch_scl);

    touch_reset.set_low();
    delay.delay_millis(10);
    touch_reset.set_high();
    delay.delay_millis(50);
    init_touch_controller(&mut i2c);

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

    let backend_state = install_platform();

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

    let window = backend_state
        .window
        .borrow()
        .as_ref()
        .cloned()
        .expect("Slint window must exist after app.show()");

    let mut consecutive_read_errors: u8 = 0;
    let mut last_touch_position: Option<slint::LogicalPosition> = None;

    loop {
        slint::platform::update_timers_and_animations();

        let sample = poll_touch_sample(&mut i2c, &mut consecutive_read_errors);
        dispatch_touch_to_slint(&window, sample, &mut last_touch_position);

        window.draw_if_needed(|software_renderer| {
            software_renderer.render_by_line(&mut renderer);
        });

        Timer::after(Duration::from_millis(TOUCH_POLL_INTERVAL_MS)).await;
    }
}
