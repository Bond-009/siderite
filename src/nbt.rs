use std::io::{Error, ErrorKind, Read, Write, Result};

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

pub trait ReadNBTExt: Read {

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
    fn read_byte_array(&mut self, lenght: usize) -> Result<Vec<i8>>
    {
        // REVIEW:
        //let len = try!(self.read_i32::<BigEndian>()) as usize;
        let mut buf = Vec::with_capacity(lenght);
        for _ in 0..lenght {
            buf.push(try!(self.read_i8()));
        }
        Ok(buf)
    }

    #[inline]
    fn read_int_array(&mut self) -> Result<Vec<i32>>
    {
        // REVIEW:
        let len = try!(self.read_i32::<BigEndian>()) as usize;
        let mut buf = Vec::with_capacity(len);
        for _ in 0..len {
            buf.push(try!(self.read_i32::<BigEndian>()));
        }
        Ok(buf)
    }

    #[inline]
    fn read_long_array(&mut self) -> Result<Vec<i64>>
    {
        // REVIEW:
        let len = self.read_i32::<BigEndian>()? as usize;
        let mut buf = Vec::with_capacity(len);
        for _ in 0..len {
            buf.push(self.read_i64::<BigEndian>()?);
        }
        Ok(buf)
    }

    #[inline]
    fn read_string(&mut self) -> Result<String>
    {
        let len = self.read_var_int().unwrap() as usize;

        if len == 0 { return Ok("".to_string()); }

        let mut bytes = vec![0; len];
        let mut n_read = 0usize;
        while n_read < bytes.len() {
            match try!(self.read(&mut bytes[n_read..])) {
                0 => return Err(Error::new(ErrorKind::InvalidInput, "Incomplete NBT value")),
                n => n_read += n
            }
        }

        match String::from_utf8(bytes){
            Ok(string) => Ok(string),
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
        Err(Error::new(ErrorKind::InvalidInput, "VarInt too big"))
    }
}

impl<R: Read + ?Sized> ReadNBTExt for R {}

pub trait WriteNBTExt: Write {

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
    fn write_byte_array(&mut self, value: &[i8]) -> Result<()>
    {
        // REVIEW:
        //try!(self.write_i32::<BigEndian>(value.len() as i32));
        for &v in value {
            try!(self.write_i8(v));
        }
        Ok(())
    }

    #[inline]
    fn write_int_array(&mut self, value: &[i32]) -> Result<()>
    {
        // REVIEW:
        try!(self.write_i32::<BigEndian>(value.len() as i32));
        for &v in value {
            try!(self.write_i32::<BigEndian>(v));
        }
        Ok(())
    }

    #[inline]
    fn write_long_array(&mut self, value: &[i64]) -> Result<()>
    {
        // REVIEW:
        self.write_i32::<BigEndian>(value.len() as i32)?;
        for &v in value {
            self.write_i64::<BigEndian>(v)?;
        }
        Ok(())
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
}

impl<W: Write + ?Sized> WriteNBTExt for W {}
