use esp_hal::gpio::Pin;
use esp_hal::peripherals::Peripherals;

use crate::embedded::ui_runtime::UiTaskResources;

pub struct BoardResources {
    pub timg0: esp_hal::peripherals::TIMG0<'static>,
    pub ui: UiTaskResources,
}

pub fn split_board_resources(peripherals: Peripherals) -> BoardResources {
    BoardResources {
        timg0: peripherals.TIMG0,
        ui: UiTaskResources {
            spi2: peripherals.SPI2,
            i2c0: peripherals.I2C0,
            lcd_sclk: peripherals.GPIO12.degrade(),
            lcd_mosi: peripherals.GPIO11.degrade(),
            lcd_miso: peripherals.GPIO13.degrade(),
            lcd_dc: peripherals.GPIO46.degrade(),
            lcd_cs: peripherals.GPIO10.degrade(),
            lcd_backlight: peripherals.GPIO45.degrade(),
            touch_reset: peripherals.GPIO18.degrade(),
            touch_sda: peripherals.GPIO16.degrade(),
            touch_scl: peripherals.GPIO15.degrade(),
        },
    }
}
