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
    frame: &'d mut [u8],
}

const LUT_VCOM0: [u8; 44] = [
    0x00, 0x17, 0x00, 0x00, 0x00, 0x02, 0x00, 0x17, 0x17, 0x00, 0x00, 0x02, 0x00, 0x0A, 0x01, 0x00,
    0x00, 0x01, 0x00, 0x0E, 0x0E, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

const LUT_WW: [u8; 42] = [
    0x40, 0x17, 0x00, 0x00, 0x00, 0x02, 0x90, 0x17, 0x17, 0x00, 0x00, 0x02, 0x40, 0x0A, 0x01, 0x00,
    0x00, 0x01, 0xA0, 0x0E, 0x0E, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

const LUT_BW: [u8; 42] = [
    0x40, 0x17, 0x00, 0x00, 0x00, 0x02, 0x90, 0x17, 0x17, 0x00, 0x00, 0x02, 0x40, 0x0A, 0x01, 0x00,
    0x00, 0x01, 0xA0, 0x0E, 0x0E, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

const LUT_BB: [u8; 42] = [
    0x80, 0x17, 0x00, 0x00, 0x00, 0x02, 0x90, 0x17, 0x17, 0x00, 0x00, 0x02, 0x80, 0x0A, 0x01, 0x00,
    0x00, 0x01, 0x50, 0x0E, 0x0E, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

const LUT_WB: [u8; 42] = [
    0x80, 0x17, 0x00, 0x00, 0x00, 0x02, 0x90, 0x17, 0x17, 0x00, 0x00, 0x02, 0x80, 0x0A, 0x01, 0x00,
    0x00, 0x01, 0x50, 0x0E, 0x0E, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

// Display resolution
pub const EPD_WIDTH: u16 = 400;
pub const EPD_HEIGHT: u16 = 300;

// Comandi EPD4IN2
const PANEL_SETTING: u8 = 0x00;
const POWER_SETTING: u8 = 0x01;
const POWER_ON: u8 = 0x04;
const BOOSTER_SOFT_START: u8 = 0x06;
const DISPLAY_REFRESH: u8 = 0x12;
const LUT_FOR_VCOM: u8 = 0x20;
const LUT_WHITE_TO_WHITE: u8 = 0x21;
const LUT_BLACK_TO_WHITE: u8 = 0x22;
const LUT_WHITE_TO_BLACK: u8 = 0x23;
const LUT_BLACK_TO_BLACK: u8 = 0x24;
const PLL_CONTROL: u8 = 0x30;
const VCOM_AND_DATA_INTERVAL_SETTING: u8 = 0x50;
const RESOLUTION_SETTING: u8 = 0x61;
const VCM_DC_SETTING: u8 = 0x82;
const DEEP_SLEEP: u8 = 0x07;
const DATA_START_TRANSMISSION_1: u8 = 0x10;
const DATA_STOP: u8 = 0x11;
const DATA_START_TRANSMISSION_2: u8 = 0x13;
const GET_STATUS: u8 = 0x71;

impl<'d> EPDMgr<'d> {
    pub fn new(
        channel: SpiDmaBus<'d, Async>,
        busy: GpioPin<6>,
        rst: GpioPin<7>,
        dc: GpioPin<8>,
        frame: &'d mut [u8],
    ) -> Self {
        Self {
            channel,
            busy: Input::new(busy, Pull::Up).into(),
            rst: Output::new(rst, Level::Low).into(),
            dc: Output::new(dc, Level::Low).into(),
            frame,
        }
    }

    fn transfer(&mut self, data: u8) {
        let mut buffer = [0; 1];
        self.channel.transfer(&mut buffer, &[data]).unwrap();
    }
    async fn reset(&mut self) {
        self.rst.set_low();
        Timer::after(Duration::from_millis(200)).await;
        self.rst.set_high();
        Timer::after(Duration::from_millis(200)).await;
    }
    async fn wait_idle(&mut self) {
        self.send_command(GET_STATUS).await;
        loop {
            //LOW: busy, HIGH: idle
            if self.busy.is_high() {
                break;
            }
            Timer::after(Duration::from_millis(100)).await;
        }
    }
    async fn send_command(&mut self, data: u8) {
        self.dc.set_low();
        self.transfer(data);
    }
    async fn send_data(&mut self, data: u8) {
        self.dc.set_high();
        self.transfer(data);
    }
    async fn set_lut(&mut self) {
        self.send_command(LUT_FOR_VCOM).await; //vcom
        for i in LUT_VCOM0.iter() {
            self.send_data(*i).await;
        }

        self.send_command(LUT_WHITE_TO_WHITE).await; //ww --
        for i in LUT_WW.iter() {
            self.send_data(*i).await;
        }

        self.send_command(LUT_BLACK_TO_WHITE).await; //bw r
        for i in LUT_BW.iter() {
            self.send_data(*i).await;
        }

        self.send_command(LUT_WHITE_TO_BLACK).await; //wb w
        for i in LUT_BB.iter() {
            self.send_data(*i).await;
        }

        self.send_command(LUT_BLACK_TO_BLACK).await; //bb b
        for i in LUT_WB.iter() {
            self.send_data(*i).await;
        }
    }

    pub async fn init(&mut self) {
        self.reset().await;

        self.send_command(POWER_SETTING).await;
        for i in [0x03, 0x00, 0x2b, 0x2b, 0xff] {
            self.send_data(i).await;
        }

        self.send_command(BOOSTER_SOFT_START).await;
        for i in [0x17, 0x17, 0x17] {
            self.send_data(i).await;
        }

        self.send_command(POWER_ON).await;
        self.wait_idle().await;

        self.send_command(PANEL_SETTING).await;
        for i in [0xbf, 0x0b] {
            self.send_data(i).await;
        }

        self.send_command(PLL_CONTROL).await;
        self.send_data(0x3c).await;

        self.send_command(RESOLUTION_SETTING).await;
        self.send_data((EPD_WIDTH >> 8) as u8).await;
        self.send_data((EPD_WIDTH & 0xff) as u8).await;
        self.send_data((EPD_HEIGHT >> 8) as u8).await;
        self.send_data((EPD_HEIGHT & 0xff) as u8).await;

        self.set_lut().await;
    }

    pub async fn display_frame(&mut self) {
        self.send_command(VCM_DC_SETTING).await;
        self.send_data(0x12).await;
        self.send_command(VCOM_AND_DATA_INTERVAL_SETTING).await;
        self.send_data(0x97).await;

        //self.send_command(DATA_START_TRANSMISSION_1).await;
        //for _ in 0..(EPD_HEIGHT * EPD_WIDTH / 2) {
        //    self.send_data(0xff).await;
        //}
        Timer::after(Duration::from_millis(2)).await;
        self.send_command(DATA_START_TRANSMISSION_2).await;
        for i in 0..self.frame.len() {
            self.send_data(self.frame[i]).await;
        }

        Timer::after(Duration::from_millis(2)).await;

        self.send_command(DISPLAY_REFRESH).await;
        Timer::after(Duration::from_millis(100)).await;
        self.wait_idle().await;
    }
    pub async fn clear(&mut self, k: u8) {
        self.frame.fill(k);
        self.display_frame().await;
    }

    //pub async fn clear(&mut self) {
    //    self.send_command(0x10).await;
    //    for _ in 0..(EPD_4IN0E_HEIGHT * EPD_4IN0E_WIDTH) {
    //        self.send_data(0).await;
    //    }
    //    self.turn_on_display().await;
    //}

    //pub async fn paint(&mut self, data: &Vec<String<16>, 16>) {
    //    self.send_command(0x10).await;
    //    for i in data.iter() {
    //        for chifer in (0..i.len()).step_by(2) {
    //            let pair = &i[chifer..chifer + 2];
    //            match u32::from_str_radix(pair, 16) {
    //                Ok(num) => {
    //                    self.send_data(0).await;
    //                    println!("in{}, n{}", pair, num);
    //                }
    //                Err(_) => println!("invalid{}", pair),
    //            }
    //        }
    //    }
    //    self.turn_on_display().await;
    //}
    //pub async fn cmd(&mut self, pkg: ParserMgr) -> Result<&'static str, &'static str> {
    //    for i in pkg.args.iter() {
    //        for chifer in (0..i.len()).step_by(2) {
    //            let pair = &i[chifer..chifer + 2];
    //            match u32::from_str_radix(pair, 16) {
    //                Ok(num) => {
    //                    self.send_command(0x10).await;
    //                    println!("in{}, n{}", pair, num);
    //                }
    //                Err(_) => println!("invalid{}", pair),
    //            }
    //        }
    //    }
    //    self.turn_on_display().await;
    //    Ok("load")
    //}
}
