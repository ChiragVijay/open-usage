//! Bounded protobuf wire reader for Antigravity accounting and OAuth envelopes.
pub enum Field<'a> {
    Integer(u64),
    Bytes(&'a [u8]),
}
fn varint(bytes: &[u8], offset: &mut usize) -> Option<u64> {
    let mut value = 0;
    for shift in 0..10 {
        let byte = *bytes.get(*offset)?;
        *offset += 1;
        if shift == 9 && byte > 1 {
            return None;
        }
        value |= u64::from(byte & 127) << (shift * 7);
        if byte & 128 == 0 {
            return Some(value);
        }
    }
    None
}
pub fn field(bytes: &[u8], requested: u64) -> Option<Field<'_>> {
    let mut offset = 0;
    while offset < bytes.len() {
        let tag = varint(bytes, &mut offset)?;
        if tag >> 3 == 0 {
            return None;
        }
        let value = match tag & 7 {
            0 => Field::Integer(varint(bytes, &mut offset)?),
            2 => {
                let len = usize::try_from(varint(bytes, &mut offset)?).ok()?;
                let end = offset.checked_add(len)?;
                let data = bytes.get(offset..end)?;
                offset = end;
                Field::Bytes(data)
            }
            1 | 5 => {
                offset = offset.checked_add(if tag & 7 == 1 { 8 } else { 4 })?;
                if offset > bytes.len() {
                    return None;
                }
                continue;
            }
            _ => return None,
        };
        if tag >> 3 == requested {
            return Some(value);
        }
    }
    None
}
pub fn bytes(data: &[u8], number: u64) -> Option<&[u8]> {
    match field(data, number)? {
        Field::Bytes(bytes) => Some(bytes),
        _ => None,
    }
}
pub fn integer(data: &[u8], number: u64) -> Option<u64> {
    match field(data, number)? {
        Field::Integer(value) => Some(value),
        _ => None,
    }
}
pub fn text(data: &[u8], number: u64) -> Option<String> {
    let text = std::str::from_utf8(bytes(data, number)?).ok()?.trim();
    (!text.is_empty()).then(|| text.to_owned())
}
