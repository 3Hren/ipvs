#[macro_use]
extern crate bitflags;
extern crate byteorder;
extern crate libc;
#[macro_use]
extern crate log;

use std::io;

mod netlink;

use netlink::Socket;

pub type Error = io::Error;

const NAME: &str = "IPVS";

#[derive(Debug)]
pub struct Client {
    sock: Socket,
    family: i32,
}

impl Client {
    pub fn new() -> Result<Client, Error> {
        let mut sock = Socket::new()?;

        let family = sock.resolve_family(NAME)?;

        let client = Client { sock, family };

        Ok(client)
    }

    /// Returns a numeric representation of netlink family that corresponds with IPVS protocol.
    pub fn family(&self) -> i32 {
        self.family
    }

    pub fn flush(&mut self) -> Result<(), Error> {
        self.sock.execute(netlink::FlushFrame)
    }
}

