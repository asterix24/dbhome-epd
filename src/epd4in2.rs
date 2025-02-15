use embassy_time::{Duration, Timer};

use esp_hal::{
    gpio::{GpioPin, Input, Level, Output, Pull},
    spi::master::SpiDmaBus,
    Async,
};

use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};

use crate::epd4in2_cmd::Command;
use crate::epd4in2_const::*;
//use crate::proto_parser::ParserMgr;

pub const EPD_WIDTH: u32 = 400;
pub const EPD_HEIGHT: u32 = 300;

pub struct EPDMgr<'d> {
    busy: Input<'d>,
    rst: Output<'d>,
    dc: Output<'d>,
    channel: SpiDmaBus<'d, Async>,
    framebuffer: [u8; EPD_WIDTH as usize * EPD_HEIGHT as usize / 8],
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
            framebuffer: [0; EPD_WIDTH as usize * EPD_HEIGHT as usize / 8],
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
        self.send_command(Command::DataStartTransmission2).await;
        for i in 0..self.framebuffer.len() {
            let px = self.framebuffer[i];
            for i in (0..8).step_by(2).rev() {
                let a = (px >> i) & 1;
                let b = (px >> i + 1) & 1;
                self.send_data(a << 4 | b).await;
            }
        }
        Timer::after(Duration::from_millis(2)).await;

        self.send_command(Command::DisplayRefresh).await;
        Timer::after(Duration::from_millis(100)).await;
        self.wait_idle().await;
    }

    pub async fn clear(&mut self, k: u8) {
        self.framebuffer.fill(k);
        self.display_frame().await;
    }

    pub async fn fill_frame(&mut self, buff: &[u8]) {
        for px in buff.into_iter() {
            for i in (0..8).step_by(2).rev() {
                let a = (px >> i) & 1;
                let b = (px >> i + 1) & 1;
                self.send_data(a << 4 | b).await;
            }
        }
    }
}

impl<'d> OriginDimensions for EPDMgr<'d> {
    fn size(&self) -> Size {
        Size::new(EPD_WIDTH as u32, EPD_HEIGHT as u32)
    }
}
impl<'d> DrawTarget for EPDMgr<'d> {
    type Color = BinaryColor;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(coord, color) in pixels.into_iter() {
            if let Ok((x @ 0..=EPD_WIDTH, y @ 0..=EPD_HEIGHT)) = coord.try_into() {
                let index = x + y * EPD_WIDTH / 8;

                let mut bits: u8 = self.framebuffer[index as usize];
                let px: u8 = 0x80 >> (x % 8);
                if color.is_on() {
                    bits |= px;
                } else {
                    bits &= !px;
                }
                self.framebuffer[index as usize] = bits;
            }
        }

        Ok(())
    }
}

/*  x ------------------->
 *  y   0 .. Width
 *  |   .   pn
 *  |   .
 *  |   Height
 *  |
 *  V
 *
 *  x * width / 8 byte x linea
 *  y
 */
