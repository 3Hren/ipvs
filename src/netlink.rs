use std::collections::HashMap;
use std::fmt::{self, Debug, Formatter};
use std::io::{Cursor, Error, Read, Write};
use std::os::unix::io::RawFd;
use std::mem;

use libc;

use byteorder::{NativeEndian, ReadBytesExt, WriteBytesExt};

bitflags! {
    struct MessageFlags: u16 {
        const REQUEST     = 0x001;
        const MULTI       = 0x002;
        const ACK         = 0x004;
        const ECHO        = 0x008;
        const DUMP_INTR   = 0x010;

        const ROOT        = 0x100;
        const MATCH       = 0x200;
        const ATOMIC      = ROOT.bits | MATCH.bits;

        const REPLACE     = 0x100;
        const EXCL        = 0x200;
        const CREATE      = 0x400;
        const APPEND      = 0x800;
        const ACK_REQUEST = REQUEST.bits | ACK.bits;
    }
}

#[derive(Debug, Default)]
pub struct Context {
    families: HashMap<&'static str, u16>,
}

impl Context {
    /// Constructs a new netlink context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a new generic netlink family, resolving its family type via asking a kernel.
    pub fn add(&mut self) -> Result<i32, Error> {
        unimplemented!();
    }
}

#[derive(Copy, Clone)]
pub struct SocketAddr {
    addr: libc::sockaddr_nl,
}

impl SocketAddr {
    pub fn new(pid: i32, groups: u32) -> SocketAddr {
        let mut addr: libc::sockaddr_nl = unsafe { mem::zeroed() };
        addr.nl_family = libc::AF_NETLINK as libc::sa_family_t;
        addr.nl_pid = pid as u32;
        addr.nl_groups = groups;

        SocketAddr { addr }
    }
}

impl Debug for SocketAddr {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        f.debug_struct("SocketAddr")
            .field("pid", &self.addr.nl_pid)
            .field("groups", &self.addr.nl_groups)
            .finish()
    }
}

#[derive(Debug)]
pub struct Socket {
    fd: RawFd,
    id: i32,
    seq: i32,
}

impl Socket {
    pub fn new() -> Result<Socket, Error> {
        let fd = unsafe {
            libc::socket(libc::AF_NETLINK, libc::SOCK_DGRAM, libc::NETLINK_GENERIC)
        };

        if fd == -1 {
            return Err(Error::last_os_error());
        }

        let addr = SocketAddr::new(0, 0);

        let ec = unsafe {
            libc::bind(fd, mem::transmute(&addr.addr), mem::size_of::<libc::sockaddr_nl>() as u32)
        };

        if ec == -1 {
            return Err(Error::last_os_error());
        }

        let sock = Socket {
            fd: fd,
            id: 0,
            seq: 0,
        };

        Ok(sock)
    }

    pub fn execute<M: Frame>(&mut self, payload: M) -> Result<(), Error> {
        let buf = vec![];
        let mut cur = Cursor::new(buf);
        payload.pack(&mut cur)?;

        let pid = unsafe { libc::getpid() };

        let len = cur.position();

        let family = payload.family();

        // TODO: Pack using serde.
        let mut vec = vec![];
        vec.write_u32::<NativeEndian>(16 + len as u32)?;
        vec.write_u16::<NativeEndian>(family)?;
        vec.write_u16::<NativeEndian>(ACK_REQUEST.bits())?;
        vec.write_i32::<NativeEndian>(self.seq)?;
        vec.write_i32::<NativeEndian>(pid)?;

        // TODO: Use I/O buffers instead to avoid copying bytes.
        vec.extend(&cur.get_ref()[..len as usize]);

        self.seq += 1;

        debug!("-> {:?}", vec);
        let rc = unsafe {
            libc::send(self.fd, vec.as_ptr() as *const libc::c_void, vec.len(), 0)
        };

        if rc == -1 {
            return Err(Error::last_os_error());
        }

        let mut rdbuf = vec![0; 16384];
        let nread = unsafe { libc::recv(self.fd, rdbuf.as_mut_ptr() as *mut libc::c_void, rdbuf.len(), 0) };
        if nread == -1 {
            return Err(Error::last_os_error());
        }

        debug!("<- {:?}", &rdbuf[..nread as usize]);

        let mut cur = Cursor::new(rdbuf);

        let header = Header::unpack(&mut cur)?;
        println!("Header=`{:?}`", header);

        let pos = cur.position();
        println!("Payload={:?}", &cur.get_ref()[pos as usize..nread as usize]);

        // Depending on header.ty unpack message.
        // ctx[ty].unpack

        // Error code.
        let ec = cur.read_i32::<NativeEndian>()?;

        // assert that only 1 message.
        // assert that this message is ErrorMessage.
        // if ec != 0 ec *= -1;
        // return OS error.

        // Deserialize again header + payload.
        let header = Header::unpack(&mut cur)?;
        println!("ec={}, {:?}", ec, header);

        // If not registered - return err.


        Ok(())
    }

    /// Resolves a netlink numeric family using its string representation.
    pub fn resolve_family(&mut self, family: &str) -> Result<i32, Error> {
        let message = ControlMessage::GetFamily(ControlAttributes {
            family_name: Some(family),
            ..Default::default()
        });

        let reply = self.execute(message)?;
        unimplemented!();
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        unsafe { libc::close(self.fd) };
    }
}

// TODO: Use serde Serialize for messages and attributes.
#[derive(Default)]
struct ControlAttributes<'a> {
    family_id: Option<u16>,
    family_name: Option<&'a str>,
}

enum ControlMessage<'a> {
    /// Returned in response to a `GetFamily` request.
    NewFamily(ControlAttributes<'a>),
    DelFamily,
    GetFamily(ControlAttributes<'a>),
}

impl<'a> ControlMessage<'a> {
    fn to_type(&self) -> u8 {
        match *self {
            ControlMessage::NewFamily(..) => 1,
            ControlMessage::DelFamily => 2,
            ControlMessage::GetFamily(..) => 3,
        }
    }
}

impl<'a> Frame for ControlMessage<'a> {
    fn family(&self) -> u16 {
        16
    }

    fn pack<W: Write>(&self, wr: &mut W) -> Result<usize, Error> {
        let ty = self.to_type();
        let version = 0x1;

        wr.write_u8(ty)?;
        wr.write_u8(version)?;
        wr.write(&[0, 0])?;

        match *self {
            ControlMessage::GetFamily(ref attributes) => {
                let l1 = if let Some(id) = attributes.family_id {
                    let len = 2 + 2 + 2;
                    wr.write_u16::<NativeEndian>(len as u16)?;
                    wr.write_u16::<NativeEndian>(0)?;
                    wr.write_u16::<NativeEndian>(id)?;
                    len
                } else {
                    0
                };

                let l2 = if let Some(name) = attributes.family_name {
                    let len = 2 + 2 + name.len() + 1;
                    wr.write_u16::<NativeEndian>(len as u16)?;
                    wr.write_u16::<NativeEndian>(2)?;
                    wr.write_all(name.as_bytes())?;
                    wr.write(&[0])?;
                    let add = 4 - (len % 4) & 0x3;
                    for i in 0..add {
                        wr.write_u8(0)?;
                    }
                    len + add
                } else {
                    0
                };

                let len = 1 + 1 + 2 + l1 + l2;

                Ok(0)
            }
            _ => unimplemented!(),
        }
    }
}

// See https://tools.ietf.org/html/rfc3549#section-2.3.2.2
struct AckMessage;

#[derive(Copy, Clone, Debug)]
pub struct Header {
    /// The length of the message in bytes, including the header.
    len: u32,
    /// Describes the message content.
    ty: u16,
    /// Additional flags.
    flags: u16,
    /// The sequence number of the message.
    seq: u32,
    /// Process ID.
    pid: u32,
}

impl Header {
    pub fn unpack<R: Read>(rd: &mut R) -> Result<Self, Error> {
        let len = rd.read_u32::<NativeEndian>()?;
        let ty = rd.read_u16::<NativeEndian>()?;
        let flags = rd.read_u16::<NativeEndian>()?;
        let seq = rd.read_u32::<NativeEndian>()?;
        let pid = rd.read_u32::<NativeEndian>()?;

        let header = Self {
            len: len,
            ty: ty,
            flags: flags,
            seq: seq,
            pid: pid,
        };

        Ok(header)
    }
}

#[derive(Clone, Debug)]
struct ErrorMessage {
    id: i32,
    reason: String,
}

impl ErrorMessage {
    pub fn unpack<R: Read>(rd: &mut R) -> Result<Self, Error> {
        let id = rd.read_i32::<NativeEndian>()?;
        let mut reason = String::new();

        rd.read_to_string(&mut reason)?;

        let err = Self { id, reason };

        Ok(err)
    }
}

pub trait Frame: Sized {
    fn family(&self) -> u16;
    fn pack<W: Write>(&self, wr: &mut W) -> Result<usize, Error>;
}

pub struct FlushFrame;

impl Frame for FlushFrame {
    fn family(&self) -> u16 {
        26
    }

    fn pack<W: Write>(&self, wr: &mut W) -> Result<usize, Error> {
        wr.write(&[17, 1, 0, 0])
    }
}

// TODO: Test GET_FAMILY IPVS -> [3, 1, 0, 0, | 9, 0, 2, 0, | 73, 80, 86, 83 |, \0, 0, 0, 0].

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn get_family_pack() {
        let message = ControlMessage::GetFamily(ControlAttributes {
            family_name: Some("IPVS"),
            ..Default::default()
        });

        let mut buf = vec![];
        message.pack(&mut buf);

        assert_eq!(&[3, 1, 0, 0, 9, 0, 2, 0, 73, 80, 86, 83, 0, 0, 0, 0][..], &buf[..]);
    }
}
