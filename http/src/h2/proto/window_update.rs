use tracing::trace;

use crate::bytes::BufMut;

use super::{
    error::Error,
    head::{Head, Kind},
    stream_id::StreamId,
    unpack_octets_4,
};

const SIZE_INCREMENT_MASK: u32 = 1 << 31;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct WindowUpdate {
    stream_id: StreamId,
    size_increment: u32,
}

impl WindowUpdate {
    pub fn new(stream_id: StreamId, size_increment: u32) -> WindowUpdate {
        WindowUpdate {
            stream_id,
            size_increment,
        }
    }

    pub fn stream_id(&self) -> StreamId {
        self.stream_id
    }

    pub fn size_increment(&self) -> u32 {
        self.size_increment
    }

    /// Builds a `WindowUpdate` frame from a raw frame.
    pub fn load(head: Head, payload: &[u8]) -> Result<WindowUpdate, Error> {
        if payload.len() != 4 {
            todo!();
            // return Err(Error::BadFrameSize);
        }

        // Clear the most significant bit, as that is reserved and MUST be ignored
        // when received.
        let size_increment = unpack_octets_4!(payload, 0, u32) & !SIZE_INCREMENT_MASK;

        if size_increment == 0 {
            todo!()
            // return Err(Error::InvalidWindowUpdateValue);
        }

        Ok(WindowUpdate {
            stream_id: head.stream_id(),
            size_increment,
        })
    }

    pub fn encode<B: BufMut>(&self, dst: &mut B) {
        trace!("encoding WINDOW_UPDATE; id={:?}", self.stream_id);
        let head = Head::new(Kind::WindowUpdate, 0, self.stream_id);
        head.encode(4, dst);
        dst.put_u32(self.size_increment);
    }
}
