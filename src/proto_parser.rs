use core::str::FromStr;

use heapless::{String, Vec};

pub struct ParserMgr {
    pub cmd: String<32>,
    pub args: Vec<String<16>, 10>,
}

impl ParserMgr {
    pub fn new(msg: String<128>) -> Self {
        let mut tokens = msg.split_whitespace();

        let mut cmd: String<32> = String::new();
        if let Some(t) = tokens.next() {
            cmd = String::try_from(t).unwrap();
        }

        let mut args: Vec<String<16>, 10> = Vec::new();
        for token in tokens {
            args.push(String::try_from(token).unwrap()).unwrap();
        }
        Self { cmd, args }
    }
}

pub fn reply_ok(payload: &str) -> String<64> {
    let mut ret = String::from_str(payload).unwrap();
    ret.push_str("\nOK\n").unwrap();
    ret
}

pub fn reply_err(payload: &str) -> String<64> {
    let mut ret = String::from_str(payload).unwrap();
    ret.push_str("\nErr\n").unwrap();
    ret
}
