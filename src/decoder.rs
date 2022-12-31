use std::io;
use bytes::{Buf, BytesMut};
use tokio_util::codec::Decoder;

pub struct Dsmr5Codec {}

impl Dsmr5Codec {
    pub fn new() -> Self {
        Dsmr5Codec {}
    }
}

impl Decoder for Dsmr5Codec {
    type Item = dsmr5::state::State;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() > 2048 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Received frame longer than max"));
        }

        if src.capacity() < 2048 {
            src.reserve(2048 - src.capacity());
        }

        match src.as_ref().iter().position(|b| *b == b'/') {
            None => return Ok(None),
            Some(index) => {
                if index > 0 {
                    src.advance(index);
                }
            },
        };

        let end_index = match src.as_ref().iter().position(|b| *b == b'!') {
            None => return Ok(None),
            Some(index) => index + 7,
        };

        if src.len() < end_index {
            return Ok(None);
        }

        let mut frame = src.split_to(end_index);
        frame.resize(2048, 0);

        let readout = dsmr5::Readout { buffer: frame.as_ref().try_into().unwrap() };
        let telegram = readout.to_telegram().map_err(|err|
            io::Error::new(io::ErrorKind::InvalidData, format!("Failed to decode telegram: {:?}", err))
        )?;
        let state = dsmr5::Result::from(&telegram).map_err(|err|
            io::Error::new(io::ErrorKind::InvalidData, format!("{:?}", err))
        )?;

        Ok(Some(state))
    }
}
