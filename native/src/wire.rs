use std::io::{self, Read, Write};
pub const MAX_FRAME: usize = 900_000;
pub fn read(reader: &mut impl Read) -> io::Result<Option<serde_json::Value>> {
    let mut header = [0u8; 4];
    let n = reader.read(&mut header[..1])?;
    if n == 0 {
        return Ok(None);
    }
    reader.read_exact(&mut header[1..])?;
    let len = u32::from_le_bytes(header) as usize;
    if len == 0 || len > MAX_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid message length",
        ));
    }
    let mut body = vec![0; len];
    reader.read_exact(&mut body)?;
    Ok(Some(serde_json::from_slice(&body)?))
}
pub fn write(writer: &mut impl Write, value: &serde_json::Value) -> io::Result<()> {
    let data = serde_json::to_vec(value)?;
    if data.len() > MAX_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Response too large",
        ));
    }
    writer.write_all(&(data.len() as u32).to_le_bytes())?;
    writer.write_all(&data)?;
    writer.flush()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn framing_roundtrip() {
        let mut b = vec![];
        write(&mut b, &serde_json::json!({"hello":"世界"})).unwrap();
        assert_eq!(read(&mut &b[..]).unwrap().unwrap()["hello"], "世界");
    }
    #[test]
    fn rejects_oversized() {
        assert!(read(&mut &u32::MAX.to_le_bytes()[..]).is_err());
    }
    #[test]
    fn rejects_truncated() {
        assert!(read(&mut &[4u8, 0][..]).is_err());
    }
}
