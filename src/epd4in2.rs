use embassy_time::{Duration, Timer};

use esp_hal::{
    gpio::{GpioPin, Input, Level, Output, Pull},
    spi::master::SpiDmaBus,
    Async,
};

//use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};
use esp_println::println;

use crate::epd4in2_cmd::Command;
use crate::epd4in2_const::*;
use crate::proto_parser::ParserMgr;

pub const EPD_WIDTH: usize = 400;
pub const EPD_HEIGHT: usize = 300;

//framebuffer: [u8; EPD_WIDTH as usize * EPD_HEIGHT as usize / 8],
pub struct EPDMgr<'d> {
    busy: Input<'d>,
    rst: Output<'d>,
    dc: Output<'d>,
    channel: SpiDmaBus<'d, Async>,
    payload: [u8; EPD_WIDTH * EPD_HEIGHT / 8],
}

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
            payload: [0xff; EPD_WIDTH * EPD_HEIGHT / 8],
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

        self.send_command(Command::VcmDcSetting).await;
        self.send_data(0x12).await;
        self.send_command(Command::VcomAndDataIntervalSetting).await;
        self.send_data(0x97).await;

        self.set_lut().await;
    }

    pub async fn display_frame(&mut self) {
        self.send_command(Command::DataStartTransmission1).await;
        for _ in 0..self.payload.len() {
            self.send_data(0xff).await;
        }
        Timer::after(Duration::from_millis(2)).await;

        self.send_command(Command::DataStartTransmission2).await;
        for idx in 0..self.payload.len() {
            self.send_data(self.payload[idx]).await;
        }
        Timer::after(Duration::from_millis(2)).await;

        self.send_command(Command::DisplayRefresh).await;
        Timer::after(Duration::from_millis(100)).await;

        self.wait_idle().await;
    }

    pub fn update_frame(&mut self, chunk: &[u8], offset: usize, size: usize) {
        if offset + size > self.payload.len() {
            println!("Frame overflow");
            return;
        }

        let free_space = self.payload.len() - offset;
        let max_bytes = core::cmp::min(free_space, size as usize);

        println!("{} {} {} {}", offset, free_space, max_bytes, size);

        self.payload[offset..(offset + max_bytes)].copy_from_slice(&chunk[..max_bytes]);
    }

    pub async fn cmd(&mut self, _pkg: ParserMgr) -> Result<&'static str, &'static str> {
        self.display_frame().await;
        Ok("Update")
    }
}
