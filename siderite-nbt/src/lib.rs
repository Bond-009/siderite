use std::io::{Error, ErrorKind, Read, Result};

use mcrw::MCReadExt;
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

#[repr(u8)]
#[derive(Copy, Clone, Debug, FromPrimitive, PartialEq)]
pub enum TagType {
    End = 0,
    Byte = 1,
    Short = 2,
    Int = 3,
    Long = 4,
    Float = 5,
    Double = 6,
    ByteArray = 7,
    String = 8,
    List = 9,
    Compound = 10
}

pub trait NBTRead: Read {
    fn read_tag(&mut self) -> Result<(TagType, String)> {
        let tag_type = TagType::from_u8(self.read_ubyte()?).ok_or(Error::new(ErrorKind::InvalidInput, "Unknown Tag Type"))?;
        let name = self.read_string()?;

        Ok((tag_type, name))
    }

    fn read_string(&mut self) -> Result<String> {
        let len = self.read_short()?;
        if len == 0 {
            return Ok(String::new());
        }

        let mut bytes = vec![0; len as usize];
        self.read_exact(&mut bytes)?;
        String::from_utf8(bytes).map_err(|_| Error::new(ErrorKind::InvalidInput, "Couldn't create string"))
    }
}

impl<R: Read + ?Sized> NBTRead for R {}
