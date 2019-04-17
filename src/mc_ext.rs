use std::io::{Error, ErrorKind, Read, Write, Result};
use std::str;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

pub trait MCReadExt: Read {

    #[inline]
    fn read_bool(&mut self) -> Result<bool>
    {
        match self.read_u8() {
            Ok(0x00) => Ok(false),
            Ok(0x01) => Ok(true),
            Ok(v) => Err(Error::new(ErrorKind::InvalidInput, format!("Invalid Value: {:#X}", v))),
            Err(e) => Err(e),
        }
    }

    #[inline]
    fn read_byte(&mut self) -> Result<i8>
    {
        self.read_i8()
    }

    #[inline]
    fn read_ubyte(&mut self) -> Result<u8>
    {
        self.read_u8()
    }

    #[inline]
    fn read_short(&mut self) -> Result<i16>
    {
        self.read_i16::<BigEndian>()
    }

    #[inline]
    fn read_ushort(&mut self) -> Result<u16>
    {
        self.read_u16::<BigEndian>()
    }

    #[inline]
    fn read_int(&mut self) -> Result<i32>
    {
        self.read_i32::<BigEndian>()
    }

    #[inline]
    fn read_long(&mut self) -> Result<i64>
    {
        self.read_i64::<BigEndian>()
    }

    #[inline]
    fn read_ulong(&mut self) -> Result<u64>
    {
        self.read_u64::<BigEndian>()
    }

    #[inline]
    fn read_float(&mut self) -> Result<f32>
    {
        self.read_f32::<BigEndian>()
    }

    #[inline]
    fn read_double(&mut self) -> Result<f64>
    {
        self.read_f64::<BigEndian>()
    }

    #[inline]
    fn read_string(&mut self) -> Result<String>
    {
        let len = self.read_var_int().unwrap();

        if len == 0 { return Ok("".to_owned()); }

        let mut bytes = vec![0; len as usize];
        /*
        let mut n_read = 0usize;
        while n_read < bytes.len() {
            match try!(self.read(&mut bytes[n_read..])) {
                0 => return Err(Error::new(ErrorKind::InvalidInput, "Incomplete NBT value")),
                n => n_read += n
            }
        }*/

        match self.read(&mut bytes) {
            Ok(_) => (),
            Err(e) => return Err(e)
        };

        match String::from_utf8(bytes) {
            Ok(res) => Ok(res),
            Err(_) => Err(Error::new(ErrorKind::InvalidInput, "Couldn't create string"))
        }
    }

    #[inline]
    fn read_var_int(&mut self) -> Result<i32> {
        let mut x = 0i32;

        for shift in [0u32, 7, 14, 21, 28].into_iter() { // (0..32).step_by(7)
            let b = try!(self.read_u8()) as i32;
            x |= (b & 0x7F) << shift;
            if (b & 0x80) == 0 {
                return Ok(x);
            }
        }

        // The number is too large to represent in a 32-bit value.
        Err(Error::new(ErrorKind::InvalidInput, "VarInt is too big"))
    }

    #[inline]
    // REVIEW
    fn read_position(&mut self) -> Result<(i64, i64, i64)> {
        let value = self.read_long()?;
        return Ok((value >> 38, (value >> 26) & 0xFFF, value << 38 >> 38));
    }
}

impl<R: Read + ?Sized> MCReadExt for R {}

pub trait MCWriteExt: Write {

    #[inline]
    fn write_bool(&mut self, value: bool) -> Result<()>
    {
        if value {
            self.write_u8(0x01)
        }
        else {
            self.write_u8(0x00)
        }
    }

    #[inline]
    fn write_byte(&mut self, value: i8) -> Result<()>
    {
        self.write_i8(value)
    }


    #[inline]
    fn write_ubyte(&mut self, value: u8) -> Result<()>
    {
        self.write_u8(value)
    }

    #[inline]
    fn write_short(&mut self, value: i16) -> Result<()>
    {
        self.write_i16::<BigEndian>(value)
    }

    #[inline]
    fn write_ushort(&mut self, value: u16) -> Result<()>
    {
        self.write_u16::<BigEndian>(value)
    }

    #[inline]
    fn write_int(&mut self, value: i32) -> Result<()>
    {
        self.write_i32::<BigEndian>(value)
    }

    #[inline]
    fn write_long(&mut self, value: i64) -> Result<()>
    {
        self.write_i64::<BigEndian>(value)
    }

    #[inline]
    fn write_ulong(&mut self, value: u64) -> Result<()>
    {
        self.write_u64::<BigEndian>(value)
    }

    #[inline]
    fn write_float(&mut self, value: f32) -> Result<()>
    {
        self.write_f32::<BigEndian>(value)
    }

    #[inline]
    fn write_double(&mut self, value: f64) -> Result<()>
    {
        self.write_f64::<BigEndian>(value)
    }

    #[inline]
    fn write_string(&mut self, value: &str) -> Result<()>
    {
        self.write_var_int(value.len() as i32)?;
        self.write_all(value.as_bytes())
    }

    #[inline]
    fn write_var_int(&mut self, value: i32) -> Result<()>
    {
        let mut temp = value as u32;
        loop {
            if (temp & !0x7fu32) == 0 {
                try!(self.write_u8(temp as u8));
                return Ok(());
            } else {
                try!(self.write_u8(((temp & 0x7F) | 0x80) as u8));
                temp >>= 7;
            }
        }
    }

    #[inline]
    // REVIEW
    fn write_position(&mut self, x: i64, y: i64, z: i64) -> Result<()> {
        let value: i64 = ((x & 0x3FFFFFF) << 38) | ((y & 0xFFF) << 26) | (z & 0x3FFFFFF);
        self.write_long(value)
    }
}

impl<W: Write + ?Sized> MCWriteExt for W {}
