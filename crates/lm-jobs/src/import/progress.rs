//! Progress and cancellation for an import: a reader that counts the bytes it reads, reports
//! them, and stops with an error once the import is canceled.

use std::io::Read;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// Shared between an import's thread and the interface.
#[derive(Clone, Default)]
pub struct Ticket {
    canceled: Arc<AtomicBool>,
    done: Arc<AtomicU64>,
    total: Arc<AtomicU64>,
}

impl Ticket {
    pub fn cancel(&self) {
        self.canceled.store(true, Ordering::Relaxed);
    }

    pub fn is_canceled(&self) -> bool {
        self.canceled.load(Ordering::Relaxed)
    }

    /// Bytes read so far and in total (0 when unknown).
    pub fn bytes(&self) -> (u64, u64) {
        (self.done.load(Ordering::Relaxed), self.total.load(Ordering::Relaxed))
    }

    pub fn set_total(&self, n: u64) {
        self.total.store(n, Ordering::Relaxed);
        self.done.store(0, Ordering::Relaxed);
    }

    fn add(&self, n: u64) {
        self.done.fetch_add(n, Ordering::Relaxed);
    }

    /// An error to return when the import was canceled.
    pub fn check(&self) -> Result<(), super::ImportError> {
        if self.is_canceled() { Err(super::ImportError::Canceled) } else { Ok(()) }
    }
}

/// The error a [`Counting`] reader returns once canceled; readers pass it up as an I/O error.
pub const CANCELED: &str = "lm-import-canceled";

/// Wraps a reader: counts bytes into the ticket and fails once the import is canceled, so even
/// an upstream reader that knows nothing of cancellation stops promptly.
pub struct Counting<R> {
    inner: R,
    ticket: Ticket,
}

impl<R> Counting<R> {
    pub fn new(inner: R, ticket: Ticket) -> Self {
        Counting { inner, ticket }
    }
}

impl<R: Read> Read for Counting<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.ticket.is_canceled() {
            return Err(std::io::Error::other(CANCELED));
        }
        let n = self.inner.read(buf)?;
        self.ticket.add(n as u64);
        Ok(n)
    }
}

impl<R: std::io::Seek> std::io::Seek for Counting<R> {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        if self.ticket.is_canceled() {
            return Err(std::io::Error::other(CANCELED));
        }
        self.inner.seek(pos)
    }
}

/// Whether an error message comes from a canceled [`Counting`] reader.
pub fn is_cancel_message(msg: &str) -> bool {
    msg.contains(CANCELED)
}

/// Decodes a byte stream in some text encoding into UTF-8 as it is read.
pub struct Decoding<R> {
    inner: R,
    decoder: encoding_rs::Decoder,
    inbuf: Vec<u8>,
    out: Vec<u8>,
    pos: usize,
    eof: bool,
}

impl<R: Read> Decoding<R> {
    pub fn new(inner: R, encoding: &'static encoding_rs::Encoding) -> Self {
        Decoding {
            inner,
            decoder: encoding.new_decoder_without_bom_handling(),
            inbuf: vec![0; 64 << 10],
            out: Vec::new(),
            pos: 0,
            eof: false,
        }
    }
}

impl<R: Read> Read for Decoding<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        while self.pos >= self.out.len() {
            if self.eof {
                return Ok(0);
            }
            let n = self.inner.read(&mut self.inbuf)?;
            self.eof = n == 0;
            let input = self.inbuf.get(..n).unwrap_or_default();
            let mut s = String::with_capacity(self.decoder.max_utf8_buffer_length(n).unwrap_or(n * 3 + 16));
            let _ = self.decoder.decode_to_string(input, &mut s, self.eof);
            self.out = s.into_bytes();
            self.pos = 0;
        }
        let rest = self.out.get(self.pos..).unwrap_or_default();
        let n = rest.len().min(buf.len());
        if let (Some(dst), Some(src)) = (buf.get_mut(..n), rest.get(..n)) {
            dst.copy_from_slice(src);
        }
        self.pos += n;
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counting_reader_counts_and_stops_when_canceled() {
        let t = Ticket::default();
        let mut r = Counting::new(&b"hello world"[..], t.clone());
        let mut buf = [0u8; 5];
        assert_eq!(r.read(&mut buf).unwrap(), 5);
        assert_eq!(t.bytes().0, 5);
        t.cancel();
        let e = r.read(&mut buf).unwrap_err();
        assert!(is_cancel_message(&e.to_string()));
    }

    /// `read_to_end` and `read_exact` retry `Interrupted` errors forever: a canceled reader must
    /// fail with another kind, or a canceled import spins (found with the GeoPackage reader).
    #[test]
    fn a_canceled_reader_stops_read_to_end() {
        let t = Ticket::default();
        t.cancel();
        let mut out = Vec::new();
        let e = Counting::new(&b"hello world"[..], t).read_to_end(&mut out).unwrap_err();
        assert!(is_cancel_message(&e.to_string()));
    }

    #[test]
    fn gbk_is_decoded_to_utf8_across_buffer_boundaries() {
        let text = "人民广场,上海\n".repeat(20_000);
        let (bytes, _, _) = encoding_rs::GBK.encode(&text);
        let mut out = String::new();
        Decoding::new(&bytes[..], encoding_rs::GBK).read_to_string(&mut out).unwrap();
        assert_eq!(out, "人民广场,上海\n".repeat(20_000));
    }
}
