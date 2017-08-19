//! Tunnel protocol
//!
//! Adding a header on paylad
//!
//! ```plain
//! +---------------+-----------------------------------------+
//! | LENGTH LE     | PAYLOAD                                 |
//! +---------------+-----------------------------------------+
//! ```
//!
//! Send `LENGTH=0` when the stream is closing

use std::io::{self, ErrorKind, Read, Write};

use bytes::{ByteOrder, LittleEndian};
use futures::{Async, Future, Poll};

#[derive(Debug, Eq, PartialEq)]
enum DecodeState {
    ReadLength,
    Read,
    ReadDone,
    Eof,
}

#[derive(Debug, Eq, PartialEq)]
enum EncodeState {
    Read,
    CopyEncoded,
    WriteEof,
    Eof,
}

fn unexpected_eof() -> io::Error {
    io::Error::new(ErrorKind::UnexpectedEof, "unexpected eof")
}

/// Read from encoded reader and copy payload to writer
pub struct TunnelCopyDecode<R, W>
where
    R: Read,
    W: Write,
{
    r: R,
    w: W,
    buf: Vec<u8>,
    cap: usize,
    pos: usize,
    amt: u64,
    state: DecodeState,
    data_length: usize,
    closing: bool,
}

impl<R, W> TunnelCopyDecode<R, W>
where
    R: Read,
    W: Write,
{
    pub fn new(r: R, w: W) -> TunnelCopyDecode<R, W> {
        TunnelCopyDecode {
            r: r,
            w: w,
            buf: vec![0u8; 2048],
            cap: 0,
            pos: 0,
            amt: 0,
            state: DecodeState::ReadLength,
            data_length: 0,
            closing: false,
        }
    }

    fn read_length(&mut self) -> Poll<(), io::Error> {
        if self.closing && self.cap == 0 && self.state != DecodeState::Eof {
            self.state = DecodeState::Eof;
            trace!("TunnelCopyDecode is closing by flag");
            return Ok(Async::Ready(()));
        }

        while self.cap < 4 {
            let n = try_nb!(self.r.read(&mut self.buf[self.cap..4]));
            if n == 0 {
                // EOF??!
                if self.cap == 0 {
                    self.state = DecodeState::Eof;
                    return Ok(Async::Ready(()));
                }
                return Err(unexpected_eof());
            }

            self.cap += n;
        }

        self.data_length = LittleEndian::read_u32(&self.buf[0..4]) as usize;
        self.cap = 0;

        if self.data_length == 0 {
            // Data length is 0, EOF packet
            self.state = DecodeState::Eof;
            trace!("TunnelCopyDecode is closing by remote");
            return Ok(Async::Ready(()));
        }

        if self.buf.len() < self.data_length {
            self.buf.resize(self.data_length, 0);
        }

        self.state = DecodeState::Read;
        Ok(Async::Ready(()))
    }

    fn read(&mut self) -> Poll<(), io::Error> {
        while self.cap < self.data_length {
            let n = try_nb!(self.r.read(&mut self.buf[self.cap..]));
            if n == 0 {
                return Err(unexpected_eof());
            }

            self.cap += n;
        }

        self.state = DecodeState::ReadDone;
        self.pos = 0;
        Ok(Async::Ready(()))
    }

    fn read_done(&mut self) -> Poll<(), io::Error> {
        while self.pos < self.cap {
            let n = try_nb!(self.w.write(&self.buf[self.pos..self.cap]));
            if n == 0 {
                return Err(unexpected_eof());
            }
            self.pos += n;
            self.amt += n as u64;
        }

        self.state = DecodeState::ReadLength;
        self.cap = 0;
        self.pos = 0;
        Ok(Async::Ready(()))
    }

    pub fn close(mut self) -> Self {
        self.closing = true;
        self
    }
}

impl<R, W> Future for TunnelCopyDecode<R, W>
where
    R: Read,
    W: Write,
{
    type Item = u64;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            match self.state {
                DecodeState::Eof => {
                    try_nb!(self.w.flush());

                    trace!("TunnelCopyDecode is EOF, amt={}", self.amt);
                    return Ok(Async::Ready(self.amt));
                }
                DecodeState::ReadLength => try_ready!(self.read_length()),
                DecodeState::Read => try_ready!(self.read()),
                DecodeState::ReadDone => try_ready!(self.read_done()),
            }
        }

    }
}

pub fn copy_decode<R, W>(r: R, w: W) -> TunnelCopyDecode<R, W>
where
    R: Read,
    W: Write,
{
    TunnelCopyDecode::new(r, w)
}

/// Read from reader and copy to encoded writer
pub struct TunnelCopyEncode<R, W>
where
    R: Read,
    W: Write,
{
    r: R,
    w: W,
    buf: Vec<u8>,
    cap: usize,
    pos: usize,
    state: EncodeState,
    amt: u64,
    closing: bool,
}

impl<R, W> TunnelCopyEncode<R, W>
where
    R: Read,
    W: Write,
{
    pub fn new(r: R, w: W) -> TunnelCopyEncode<R, W> {
        TunnelCopyEncode {
            r: r,
            w: w,
            buf: vec![0u8; 2048 + 4],
            cap: 0,
            pos: 0,
            state: EncodeState::Read,
            amt: 0,
            closing: false,
        }
    }

    fn read(&mut self) -> Poll<(), io::Error> {
        if self.closing && self.state != EncodeState::Eof {
            LittleEndian::write_u32(&mut self.buf[0..4], 0);
            self.state = EncodeState::WriteEof;
            self.cap = 0;
            self.pos = 0;
            trace!("TunnelCopyEncode is closing by flag");
            return Ok(Async::Ready(()));
        }

        let n = try_nb!(self.r.read(&mut self.buf[4..]));
        LittleEndian::write_u32(&mut self.buf[0..4], n as u32);
        self.cap = n + 4;
        self.pos = 0;

        if n == 0 {
            // EOF!!
            self.state = EncodeState::WriteEof;
            trace!("TunnelCopyEncode is closing by remote");
            return Ok(Async::Ready(()));
        }

        self.state = EncodeState::CopyEncoded;
        Ok(Async::Ready(()))
    }

    fn copy_encoded(&mut self, to_state: EncodeState) -> Poll<(), io::Error> {
        while self.pos < self.cap {
            let n = try_nb!(self.w.write(&self.buf[self.pos..self.cap]));
            if n == 0 {
                return Err(unexpected_eof());
            }
            self.pos += n;
            self.amt += n as u64;
        }

        self.state = to_state;
        self.cap = 0;
        self.pos = 0;
        Ok(Async::Ready(()))
    }

    pub fn close(mut self) -> Self {
        if self.state == EncodeState::Eof {
            return self;
        }

        self.closing = true;
        self
    }
}

impl<R, W> Future for TunnelCopyEncode<R, W>
where
    R: Read,
    W: Write,
{
    type Item = u64;
    type Error = io::Error;
    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            match self.state {
                EncodeState::Read => try_ready!(self.read()),
                EncodeState::CopyEncoded => try_ready!(self.copy_encoded(EncodeState::Read)),
                EncodeState::WriteEof => try_ready!(self.copy_encoded(EncodeState::Eof)),
                EncodeState::Eof => {
                    try_nb!(self.w.flush());

                    trace!("TunnelCopyEncode is EOF, amt={}", self.amt);
                    return Ok(Async::Ready(self.amt));
                }
            }
        }
    }
}

pub fn copy_encode<R, W>(r: R, w: W) -> TunnelCopyEncode<R, W>
where
    R: Read,
    W: Write,
{
    TunnelCopyEncode::new(r, w)
}
