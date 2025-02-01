use embassy_time::{Duration, Timer};

use esp_hal::{
    gpio::{GpioPin, Input, Level, Output, Pull},
    spi::master::SpiDmaBus,
    Async,
};

use crate::proto_parser::ParserMgr;

/// Default Background Color
//pub const DEFAULT_BACKGROUND_COLOR: Color = Color::White;

//The Lookup Tables for the Display
use crate::epd4in2_cmd::Command;
use crate::epd4in2_const::*;

pub struct EPDMgr<'d> {
    busy: Input<'d>,
    rst: Output<'d>,
    dc: Output<'d>,
    channel: SpiDmaBus<'d, Async>,
    frame: &'d mut [u8],
}

// Display resolution
pub const EPD_WIDTH: u16 = 400;
pub const EPD_HEIGHT: u16 = 300;

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
        self.send_command(Command::GetStatus).await;
        loop {
            //LOW: busy, HIGH: idle
            if self.busy.is_high() {
                break;
            }
            Timer::after(Duration::from_millis(100)).await;
        }
    }
    async fn send_command(&mut self, cmd: Command) {
        self.dc.set_low();
        self.transfer(cmd.address());
    }
    async fn send_data(&mut self, data: u8) {
        self.dc.set_high();
        self.transfer(data);
    }
    async fn set_lut(&mut self) {
        self.send_command(Command::LutForVcom).await; //vcom
        for i in LUT_VCOM0.iter() {
            self.send_data(*i).await;
        }

        self.send_command(Command::LutWhiteToWhite).await; //ww --
        for i in LUT_WW.iter() {
            self.send_data(*i).await;
        }

        self.send_command(Command::LutBlackToWhite).await; //bw r
        for i in LUT_BW.iter() {
            self.send_data(*i).await;
        }

        self.send_command(Command::LutWhiteToBlack).await; //wb w
        for i in LUT_BB.iter() {
            self.send_data(*i).await;
        }

        self.send_command(Command::LutBlackToBlack).await; //bb b
        for i in LUT_WB.iter() {
            self.send_data(*i).await;
        }
    }

    pub async fn init(&mut self) {
        self.reset().await;

        self.send_command(Command::PowerSetting).await;
        for i in [0x03, 0x00, 0x2b, 0x2b, 0xff] {
            self.send_data(i).await;
        }

        self.send_command(Command::BoosterSoftStart).await;
        for i in [0x17, 0x17, 0x17] {
            self.send_data(i).await;
        }

        self.send_command(Command::PowerOn).await;
        self.wait_idle().await;

        self.send_command(Command::PanelSetting).await;
        for i in [0xbf, 0x0b] {
            self.send_data(i).await;
        }

        self.send_command(Command::PllControl).await;
        self.send_data(0x3c).await;

        self.send_command(Command::ResolutionSetting).await;
        self.send_data((EPD_WIDTH >> 8) as u8).await;
        self.send_data((EPD_WIDTH & 0xff) as u8).await;
        self.send_data((EPD_HEIGHT >> 8) as u8).await;
        self.send_data((EPD_HEIGHT & 0xff) as u8).await;

        self.set_lut().await;
    }

    pub async fn display_frame(&mut self) {
        self.send_command(Command::VcmDcSetting).await;
        self.send_data(0x12).await;
        self.send_command(Command::VcomAndDataIntervalSetting).await;
        self.send_data(0x97).await;

        self.send_command(Command::DataStartTransmission2).await;
        for i in 0..self.frame.len() {
            self.send_data(self.frame[i]).await;
        }
        Timer::after(Duration::from_millis(2)).await;

        self.send_command(Command::DisplayRefresh).await;
        Timer::after(Duration::from_millis(100)).await;
        self.wait_idle().await;
    }

    pub async fn clear(&mut self, k: u8) {
        self.frame.fill(k);
        self.display_frame().await;
    }
    pub async fn draw(&mut self) {}

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
