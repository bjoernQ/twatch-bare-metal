#![no_std]
#![no_main]

use axp20x::{Axpxx, Power};
use display_interface_spi::SPIInterfaceNoCS;
use embedded_graphics::{
    draw_target::DrawTarget,
    image::{Image, ImageRawLE},
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::Rgb565,
    prelude::{Point, Primitive, RgbColor, Size},
    primitives::{PrimitiveStyleBuilder, Rectangle},
    text::Text,
    Drawable,
};
use esp32_hal::{gpio::IO, i2c::I2C, pac::Peripherals, prelude::*, Delay, RtcCntl, Timer};
use esp_backtrace as _;
use esp_println::println;
use st7789::{Orientation, ST7789};
use xtensa_lx_rt::entry;

#[entry]
fn main() -> ! {
    let mut peripherals = Peripherals::take().unwrap();

    // Disable the watchdog timers. For the ESP32-C3, this includes the Super WDT,
    // the RTC WDT, and the TIMG WDTs.
    let mut rtc_cntl = RtcCntl::new(peripherals.RTC_CNTL);
    let mut timer0 = Timer::new(peripherals.TIMG0);

    timer0.disable();
    rtc_cntl.set_wdt_global_enable(false);

    let io = IO::new(peripherals.GPIO, peripherals.IO_MUX);
    let sclk = io.pins.gpio18;
    let miso = io.pins.gpio24; // NA
    let mosi = io.pins.gpio19;
    let cs = io.pins.gpio5;
    let dc = io.pins.gpio27.into_push_pull_output();
    let rst = io.pins.gpio3.into_push_pull_output(); // NA?

    let mut backlight = io.pins.gpio12.into_push_pull_output();
    backlight.set_high().ok();

    // MOTOR = IO4
    let mut _motor = io.pins.gpio4.into_push_pull_output();

    let i2c = I2C::new(
        peripherals.I2C0,
        io.pins.gpio21,
        io.pins.gpio22,
        10u32.kHz(),
        &mut peripherals.DPORT,
    )
    .unwrap();

    let mut axp = Axpxx::new(i2c);
    axp.init().unwrap();

    let mut delay = Delay::new();

    println!("{:?}", axp.is_acin_present());
    println!("{:?}", axp.is_vbus_present());

    // power up backlight! if powered down there is not much to see :)
    axp.set_power_output(Power::Ldo2, axp20x::PowerState::On, &mut delay)
        .ok();

    let spi = esp32_hal::spi::Spi::new(
        peripherals.SPI2,
        sclk,
        mosi,
        miso,
        cs,
        26u32.MHz(),
        embedded_hal::spi::MODE_0,
        &mut peripherals.DPORT,
    );

    // display interface abstraction from SPI and DC
    let di = SPIInterfaceNoCS::new(spi, dc);

    // create driver
    let mut display = ST7789::new(di, rst, 240, 240);

    // initialize
    display.init(&mut delay).unwrap();

    // set default orientation
    display
        .set_orientation(Orientation::PortraitSwapped)
        .unwrap();

    let raw_image_data = ImageRawLE::new(include_bytes!("../assets/ferris.raw"), 86);
    let ferris = Image::new(&raw_image_data, Point::new(34, 80));

    // draw image on black background
    display.clear(Rgb565::BLACK).unwrap();
    ferris.draw(&mut display).unwrap();

    let style = MonoTextStyle::new(&FONT_6X10, Rgb565::WHITE);

    let clear_style = PrimitiveStyleBuilder::new()
        .stroke_color(Rgb565::BLACK)
        .fill_color(Rgb565::BLACK)
        .build();

    let mut show_hello = false;
    let mut previous = false;

    loop {
        let axp_irq = axp.read_irq().unwrap();
        if !axp_irq.is_none() {
            println!("Hello! {:x?}", axp_irq);
            show_hello = !show_hello;
        }

        if show_hello {
            Text::new("Hello Rust!", Point::new(30, 180), style)
                .draw(&mut display)
                .unwrap();
        } else {
            Rectangle::new(Point::new(0, 170), Size::new(100, 30))
                .into_styled(clear_style)
                .draw(&mut display)
                .unwrap();
        }

        let vbus = axp.is_vbus_present().unwrap();
        if vbus != previous {
            if vbus {
                Text::new("*", Point::new(30, 220), style)
                    .draw(&mut display)
                    .unwrap();
            } else {
                Rectangle::new(Point::new(20, 210), Size::new(30, 30))
                    .into_styled(clear_style)
                    .draw(&mut display)
                    .unwrap();
            }

            previous = vbus;
        }

        continue; // keep optimizer from removing in --release
    }
}
