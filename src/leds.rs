use core::result::Result;
use esp_hal::gpio::{GpioPin, Level, Output};

use crate::proto_parser::ParserMgr;

pub struct LedsMgr<'d> {
    red: Output<'d>,
    green: Output<'d>,
    blue: Output<'d>,
}

impl<'d> LedsMgr<'d> {
    pub fn new(red: GpioPin<3>, green: GpioPin<4>, blue: GpioPin<5>) -> Self {
        Self {
            red: Output::new(red, Level::Low).into(),
            green: Output::new(green, Level::Low).into(),
            blue: Output::new(blue, Level::Low).into(),
        }
    }

    pub fn get_led(&mut self, label: &str) -> Result<&mut Output<'d>, &'static str> {
        let ret = match label {
            "red" => Ok(&mut self.red),
            "green" => Ok(&mut self.green),
            "blue" => Ok(&mut self.blue),
            _ => Err("ivalid label"),
        };

        ret
    }

    pub fn cmd(&mut self, pkg: ParserMgr) -> Result<&'static str, &'static str> {
        if pkg.args.len() < 1 {
            return Err("invalid args number");
        }

        match self.get_led(pkg.args[0].as_str()) {
            Ok(o) => match pkg.args[1].as_str() {
                "on" => {
                    o.set_high();
                    Ok("On")
                }
                "off" => {
                    o.set_low();
                    Ok("Off")
                }
                _ => Err("Wrong args"),
            },
            Err(e) => Err(e),
        }
    }
}
