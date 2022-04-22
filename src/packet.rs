use super::error;
use super::options::Options;
use super::OpCode;
use bytes::{Buf, BufMut, Bytes, BytesMut};

#[derive(Debug)]
pub struct Request {
    op_code: OpCode,
    filename: String,
    mode: String,
    options: Options,
}

impl Request {
    pub fn rrq(filename: &str, mode: &str, options: &Options) -> Request {
        Request {
            op_code: OpCode::Rrq,
            filename: filename.to_string(),
            mode: mode.to_string(),
            options: options.clone(),
        }
    }

    pub fn wrq(filename: &str, mode: &str, options: &Options) -> Request {
        Request {
            op_code: OpCode::Wrq,
            filename: filename.to_string(),
            mode: mode.to_string(),
            options: options.clone(),
        }
    }

    pub fn op_code(&self) -> &OpCode {
        &self.op_code
    }

    pub fn filename(&self) -> &str {
        &self.filename
    }

    pub fn mode(&self) -> &str {
        &self.mode
    }

    pub fn options(&self) -> &Options {
        &self.options
    }

    pub fn options_mut(&mut self) -> &mut Options {
        &mut self.options
    }
}

pub struct Error {
    error_code: u16,
    message: String,
}

impl Error {
    pub fn error_code(&self) -> u16 {
        self.error_code
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

pub fn parse_blocknum(buf: &mut Bytes) -> Result<u16, error::Error> {
    if buf.len() < 2 {
        return Err(error::Error::InvalidPacketLength);
    }

    Ok(buf.get_u16())
}

pub fn parse_error(buf: &mut Bytes) -> Result<Error, error::Error> {
    if buf.len() < 3 {
        return Err(error::Error::InvalidPacketLength);
    }

    let error_code = buf.get_u16();

    let mut parameters = buf.split(|&b| b == 0);
    let message = parameters.next().ok_or(error::Error::MissingErrorMessage)?;

    let message = String::from_utf8(message.into())?;

    Ok(Error {
        error_code,
        message,
    })
}

pub fn parse_oack(buf: &mut Bytes) -> Result<Options, error::Error> {
    Ok(Options::from(buf))
}

pub fn parse_opcode<T: Buf>(buf: &mut T) -> Result<Option<OpCode>, error::Error> {
    if buf.remaining() < 2 {
        return Err(error::Error::InvalidPacketLength);
    }

    let op_code = match buf.get_u16() {
        1 => Some(OpCode::Rrq),
        2 => Some(OpCode::Wrq),
        3 => Some(OpCode::Data),
        4 => Some(OpCode::Ack),
        5 => Some(OpCode::Error),
        6 => Some(OpCode::Oack),
        _ => None,
    };

    Ok(op_code)
}

pub fn parse_request(buf: &mut Bytes) -> Result<Request, error::Error> {
    if buf.len() < 6 {
        return Err(error::Error::InvalidPacketLength);
    }

    let op_code = parse_opcode(buf)?.ok_or(error::Error::InvalidOpCode)?;

    let mut parameters = buf.split(|&b| b == 0);

    let filename = parameters.next().ok_or(error::Error::MissingFileName)?;
    let filename = String::from_utf8(filename.into())?;

    let mode = parameters.next().ok_or(error::Error::MissingMode)?;
    let mode = String::from_utf8(mode.into())?;

    match mode.to_lowercase().as_str() {
        "netascii" | "octet" | "mail" => {}
        _ => {
            return Err(error::Error::InvalidMode);
        }
    }

    let options = Options::from(buf);

    Ok(Request {
        op_code,
        filename,
        mode,
        options,
    })
}

pub fn ack(blocknum_ack: u16) -> Bytes {
    let mut bytes = BytesMut::new();
    bytes.put_u16(OpCode::Ack as u16);
    bytes.put_u16(blocknum_ack);
    bytes.freeze()
}

pub fn data<T: Buf>(num: u16, data: T) -> Bytes {
    let mut bytes = BytesMut::new();
    bytes.put_u16(OpCode::Data as u16);
    bytes.put_u16(num);
    bytes.put(data);
    bytes.freeze()
}

pub fn error(err: error::Error) -> Bytes {
    let mut bytes = BytesMut::new();
    bytes.put_u16(OpCode::Error as u16);
    bytes.put_u16(err.error_code() as u16);
    bytes.put(format!("{:?}", err).as_bytes());
    bytes.put_u8(0);
    bytes.freeze()
}

pub fn oack(options: &Options) -> Bytes {
    let mut bytes = BytesMut::new();
    bytes.put_u16(OpCode::Oack as u16);
    bytes.put(options.as_bytes());
    bytes.freeze()
}

pub fn request(req: &Request) -> Bytes {
    let mut bytes = BytesMut::new();
    bytes.put_u16(req.op_code().clone() as u16);
    bytes.put(req.filename().as_bytes());
    bytes.put_u8(0);
    bytes.put(req.mode().as_bytes());
    bytes.put_u8(0);
    bytes.put(req.options().as_bytes());
    bytes.freeze()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_blocknum_less_len() {
        let mut buf = Bytes::from(&[0][..]);
        let ret = parse_blocknum(&mut buf);
        assert!(ret.is_err());
    }

    #[test]
    fn parse_blocknum_ok() -> Result<(), error::Error> {
        let mut buf = Bytes::from(&[0, 1][..]);
        let ret = parse_blocknum(&mut buf)?;
        assert_eq!(1, ret);
        Ok(())
    }

    #[test]
    fn parse_error_less_len() {
        let mut buf = Bytes::from(&[0, 1][..]);
        let ret = parse_error(&mut buf);
        assert!(ret.is_err());
    }

    #[test]
    fn parse_error_ok() -> Result<(), error::Error> {
        let mut buf = Bytes::from(&[0, 1, 0][..]);
        let ret = parse_error(&mut buf)?;
        assert_eq!(1, ret.error_code());
        assert_eq!("", ret.message());
        Ok(())
    }

    #[test]
    fn parse_request_less_len() {
        let mut buf = Bytes::from(&[0, 1, 97, 0, 4][..]);
        let ret = parse_request(&mut buf);
        assert!(ret.is_err());
    }

    #[test]
    fn parse_request_ok() -> Result<(), error::Error> {
        let mut buf = Bytes::from(&[0, 1, 97, 0, 111, 99, 116, 101, 116, 0][..]);
        let ret = parse_request(&mut buf)?;
        assert_eq!(OpCode::Rrq as u16, ret.op_code().clone() as u16);
        assert_eq!("a", ret.filename());
        assert_eq!("octet", ret.mode());
        assert!(!ret.options().has_option());
        Ok(())
    }

    #[test]
    fn parse_request_ok_with_options() -> Result<(), error::Error> {
        let mut buf = Bytes::from(
            &[
                0, 1, 97, 0, 111, 99, 116, 101, 116, 0, 98, 108, 107, 115, 105, 122, 101, 0, 56, 0,
            ][..],
        );
        let ret = parse_request(&mut buf)?;
        assert_eq!(OpCode::Rrq as u16, ret.op_code().clone() as u16);
        assert_eq!("a", ret.filename());
        assert_eq!("octet", ret.mode());
        assert_eq!(8, ret.options().blksize());
        Ok(())
    }
}
