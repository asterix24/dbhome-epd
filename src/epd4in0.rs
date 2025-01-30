use core::result::Result;
use embassy_time::{Duration, Timer};
use esp_println::println;

use esp_hal::{
    gpio::{GpioPin, Input, Level, Output, Pull},
    spi::master::SpiDmaBus,
    Async,
};
use heapless::{String, Vec};

use crate::proto_parser::ParserMgr;

pub struct EPDMgr<'d> {
    busy: Input<'d>,
    rst: Output<'d>,
    dc: Output<'d>,
    channel: SpiDmaBus<'d, Async>,
}

// Display resolution
pub const EPD_WIDTH: u16 = 400;
pub const EPD_HEIGHT: u16 = 300;

// Comandi EPD4IN2
const PANEL_SETTING: u8 = 0x00;
const POWER_SETTING: u8 = 0x01;
const POWER_ON: u8 = 0x04;
const BOOSTER_SOFT_START: u8 = 0x06;
const DISPLAY_REFRESH: u8 = 0x12;
const VCOM_AND_DATA_INTERVAL_SETTING: u8 = 0x50;
const RESOLUTION_SETTING: u8 = 0x61;
const VCM_DC_SETTING: u8 = 0x82;
const DEEP_SLEEP: u8 = 0x07;
const GET_STATUS: u8 = 0x71;

impl<'d> EPDMgr<'d> {
    pub fn new(
        channel: SpiDmaBus<'d, Async>,
        busy: GpioPin<6>,
        rst: GpioPin<7>,
        dc: GpioPin<8>,
    ) -> Self {
        Self {
            channel,
            busy: Input::new(busy, Pull::Up).into(),
            rst: Output::new(rst, Level::Low).into(),
            dc: Output::new(dc, Level::Low).into(),
        }
    }

    fn transfer(&mut self, data: u8) {
        let mut buffer = [0; 1];
        self.channel.transfer(&mut buffer, &[data]).unwrap();
    }
    async fn reset(&mut self) {
        self.rst.set_high();
        Timer::after(Duration::from_millis(20)).await;
        self.rst.set_low();
        Timer::after(Duration::from_millis(2)).await;
        self.rst.set_high();
        Timer::after(Duration::from_millis(20)).await;
    }
    async fn wait_idle(&mut self) {
        loop {
            //LOW: busy, HIGH: idle
            if self.busy.is_high() {
                break;
            }
            Timer::after(Duration::from_millis(10)).await;
        }
        Timer::after(Duration::from_millis(200)).await;
    }
    async fn send_command(&mut self, data: u8) {
        self.dc.set_low();
        self.transfer(data);
    }
    async fn send_data(&mut self, data: u8) {
        self.dc.set_high();
        self.transfer(data);
    }
    async fn turn_on_display(&mut self) {
        //Power on
        self.send_command(0x04).await;
        self.wait_idle().await;
        Timer::after(Duration::from_millis(200)).await;

        // second setting
        self.send_command(0x06).await;
        for i in [0x6F, 0x1F, 0x17, 0x27] {
            self.send_data(i).await;
        }
        Timer::after(Duration::from_millis(200)).await;

        // display refresh
        self.send_command(0x12).await;
        self.send_data(0x00).await;
        self.wait_idle().await;

        // power off
        self.send_command(0x02).await;
        self.send_data(0x00).await;
        self.wait_idle().await;

        Timer::after(Duration::from_millis(200)).await;
    }

    pub async fn init(&mut self) {
        self.reset().await;
        self.wait_idle().await;
        Timer::after(Duration::from_millis(30)).await;

        //CMDH
        self.send_command(0xAA).await;
        for i in [0x49, 0x55, 0x20, 0x08, 0x09, 0x18] {
            self.send_data(i).await;
        }

        self.send_command(0x01).await;
        for i in [0x3f] {
            self.send_data(i).await;
        }

        self.send_command(0x00).await;
        for i in [0x5f, 0x69] {
            self.send_data(i).await;
        }

        self.send_command(0x05).await;
        for i in [0x40, 0x1F, 0x1F, 0x2C] {
            self.send_data(i).await;
        }

        self.send_command(0x08).await;
        for i in [0x6F, 0x1F, 0x1F, 0x22] {
            self.send_data(i).await;
        }

        self.send_command(0x06).await;
        for i in [0x6F, 0x1F, 0x17, 0x17] {
            self.send_data(i).await;
        }

        self.send_command(0x03).await;
        for i in [0x00, 0x54, 0x00, 0x44] {
            self.send_data(i).await;
        }

        self.send_command(0x60).await;
        for i in [0x02, 0x00] {
            self.send_data(i).await;
        }

        self.send_command(0x30).await;
        for i in [0x08] {
            self.send_data(i).await;
        }

        self.send_command(0x50).await;
        for i in [0x3f] {
            self.send_data(i).await;
        }

        self.send_command(0x61).await;
        for i in [0x01, 0x90, 0x01, 0x2C] {
            self.send_data(i).await;
        }

        self.send_command(0xe3).await;
        for i in [0x2f] {
            self.send_data(i).await;
        }

        self.send_command(0x84).await;
        for i in [0x01] {
            self.send_data(i).await;
        }

        self.wait_idle().await;

        Timer::after(Duration::from_millis(200)).await;
    }
    pub async fn clear(&mut self) {
        self.send_command(0x10).await;
        for _ in 0..(EPD_4IN0E_HEIGHT * EPD_4IN0E_WIDTH) {
            self.send_data(0).await;
        }
        self.turn_on_display().await;
    }

    pub async fn paint(&mut self, data: &Vec<String<16>, 16>) {
        self.send_command(0x10).await;
        for i in data.iter() {
            for chifer in (0..i.len()).step_by(2) {
                let pair = &i[chifer..chifer + 2];
                match u32::from_str_radix(pair, 16) {
                    Ok(num) => {
                        self.send_data(0).await;
                        println!("in{}, n{}", pair, num);
                    }
                    Err(_) => println!("invalid{}", pair),
                }
            }
        }
        self.turn_on_display().await;
    }
    pub async fn cmd(&mut self, pkg: ParserMgr) -> Result<&'static str, &'static str> {
        for i in pkg.args.iter() {
            for chifer in (0..i.len()).step_by(2) {
                let pair = &i[chifer..chifer + 2];
                match u32::from_str_radix(pair, 16) {
                    Ok(num) => {
                        self.send_command(0x10).await;
                        println!("in{}, n{}", pair, num);
                    }
                    Err(_) => println!("invalid{}", pair),
                }
            }
        }
        self.turn_on_display().await;
        Ok("load")
    }
}
