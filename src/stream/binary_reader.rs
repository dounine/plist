use crate::error::Error;
use crate::plist::Plist;
use chrono::{DateTime, Utc};
use nom::IResult;
use nom::Parser;
use nom::bytes::complete::{tag, take};
use nom::combinator::{map, recognize};
use nom::multi::count;
use nom::number::complete::{be_f32, be_f64, be_u8, be_u16, be_u32, be_u64};
use std::io::Cursor;

#[derive(Debug)]
struct Trailer {
    offset_table_offset_size: u8,
    object_ref_size: u8,
    num_objects: u64,
    top_object_offset: u64,
    offset_table_start: u64,
}
#[derive(Debug)]
pub struct BinaryReader {}
impl BinaryReader {
    fn parse_bplist_header(input: &[u8]) -> IResult<&[u8], ()> {
        let (input, _) = tag("bplist00").parse(input)?;
        Ok((input, ()))
    }
    //解析尾部信息
    fn parse_trailer(input: &[u8]) -> IResult<&[u8], Trailer> {
        let (
            input,
            (
                _,
                _,
                offset_table_offset_size,
                object_ref_size,
                num_objects,
                top_object_offset,
                offset_table_start,
            ),
        ) = (
            take(4u8), //未使用的4个字节
            take(2u8), //排序版本
            be_u8,
            be_u8,
            be_u64,
            be_u64,
            be_u64,
        )
            .parse(input)?;
        Ok((
            input,
            Trailer {
                offset_table_offset_size,
                object_ref_size,
                num_objects,
                top_object_offset,
                offset_table_start,
            },
        ))
    }
    //解析对象头
    fn parse_header(input: &[u8]) -> IResult<&[u8], (u8, u8)> {
        let (input, header) = be_u8.parse(input)?;
        let object_type = (header >> 4) & 0x0F;
        let extra_info = header & 0x0F;
        Ok((input, (object_type, extra_info)))
    }
    fn parse_integer(input: &[u8], extra_info: u8) -> IResult<&[u8], Plist> {
        let size = 1 << extra_info;
        match size {
            1 => map(be_u8, |v| Plist::Integer(v as i64)).parse(input),
            2 => map(be_u16, |v| Plist::Integer(v as i64)).parse(input),
            4 => map(be_u32, |v| Plist::Integer(v as i64)).parse(input),
            8 => map(be_u64, |v| Plist::Integer(v as i64)).parse(input),
            _ => Err(nom::Err::Error(nom::error::Error::new(
                input,
                nom::error::ErrorKind::Switch,
            ))),
        }
    }
    fn parse_ascii_string(input: &[u8], extra_info: u8) -> IResult<&[u8], Plist> {
        let (input, len) = if extra_info == 0xF {
            Self::parse_count(input)?
        } else {
            (input, extra_info as usize)
        };
        let mut raw_utf16: Vec<u16> = vec![];
        let mut input = input;
        for _ in 0..len {
            let value: u16;
            (input, value) = be_u16.parse(input)?;
            raw_utf16.push(value);
        }
        let str_value = String::from_utf16(&raw_utf16).map_err(|_| {
            nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Fail))
        })?;
        Ok((input, Plist::String(str_value)))
    }
    fn parse_string(input: &[u8], extra_info: u8) -> IResult<&[u8], Plist> {
        let (input, len) = if extra_info == 0xF {
            Self::parse_count(input)?
        } else {
            (input, extra_info as usize)
        };
        let (input, str_bytes) = take(len).parse(input)?;
        let str_value = String::from_utf8_lossy(str_bytes).to_string();
        Ok((input, Plist::String(str_value)))
    }
    fn parse_offset_table(input: &[u8], counts: u64, int_size: u8) -> IResult<&[u8], Vec<usize>> {
        let counts = counts as usize;
        match int_size {
            1 => count(map(be_u8, |v| v as usize), counts).parse(input),
            2 => count(map(be_u16, |v| v as usize), counts).parse(input),
            4 => count(map(be_u32, |v| v as usize), counts).parse(input),
            8 => count(map(be_u64, |v| v as usize), counts).parse(input),
            _ => panic!("Invalid offset int size"),
        }
    }
    pub fn parse(input: &[u8]) -> IResult<&[u8], Plist> {
        let (_, _) = Self::parse_bplist_header(input)?;
        let (_, trailer) = Self::parse_trailer(&input[input.len() - 32..])?;
        let offset_table_start = trailer.offset_table_start as usize;
        let (_, offsets) = Self::parse_offset_table(
            &input[offset_table_start..],
            trailer.num_objects,
            trailer.offset_table_offset_size,
        )?;
        let offset = offsets[trailer.top_object_offset as usize];
        Self::parse_object(input, offset, &offsets, &trailer)
    }
    fn parse_float(input: &[u8], extra_info: u8) -> IResult<&[u8], Plist> {
        match extra_info {
            0 => map(be_f32, |v| Plist::Float(v as f64)).parse(input),
            2 => map(be_f32, |v| Plist::Float(v as f64)).parse(input),
            3 => map(be_f64, |v| Plist::Float(v as f64)).parse(input),
            _ => Err(nom::Err::Error(nom::error::Error::new(
                input,
                nom::error::ErrorKind::Switch,
            ))),
        }
    }
    fn parse_bool(input: &[u8], extra_info: u8) -> IResult<&[u8], Plist> {
        Ok((
            input,
            match extra_info {
                0x00 => Plist::Boolean(false),
                0x08 => Plist::Boolean(false),
                0x09 => Plist::Boolean(true),
                _ => {
                    return Err(nom::Err::Failure(nom::error::Error::new(
                        input,
                        nom::error::ErrorKind::TooLarge,
                    )));
                }
            },
        ))
    }
    fn parse_date(input: &[u8], _extra_info: u8) -> IResult<&[u8], Plist> {
        let (input, timestamp) = recognize(be_f64).parse(input)?;
        let bytes: [u8; 8] = timestamp.try_into().unwrap();
        let seconds_since_2001 = f64::from_be_bytes(bytes);
        let unix_timestamp = seconds_since_2001 + 978307200.0;

        // 5. 转换为 DateTime<Utc>
        let naive =
            DateTime::from_timestamp(unix_timestamp as i64, (unix_timestamp.fract() * 1e9) as u32)
                .unwrap();
        let datetime = DateTime::<Utc>::from(naive);
        Ok((input, Plist::Date(datetime)))
    }
    fn parse_count(input: &[u8]) -> IResult<&[u8], usize> {
        let (input, header) = be_u8.parse(input)?;
        let byte_count = 1 << (header & 0x0F);
        match byte_count {
            1 => map(be_u8, |v| v as usize).parse(input),
            2 => map(be_u16, |v| v as usize).parse(input),
            4 => map(be_u32, |v| v as usize).parse(input),
            8 => map(be_u64, |v| v as usize).parse(input),
            _ => Err(nom::Err::Failure(nom::error::Error::new(
                input,
                nom::error::ErrorKind::TooLarge,
            ))),
        }
    }
    fn parse_data(input: &[u8], extra_info: u8) -> IResult<&[u8], Plist> {
        let (input, len) = if extra_info == 0xF {
            Self::parse_count(input)?
        } else {
            (input, extra_info as usize)
        };
        let (input, data) = take(len).parse(input)?;
        Ok((input, Plist::Data(data.to_vec())))
    }
    fn parse_array<'a>(
        data: &'a [u8],
        offset: usize,
        extra_info: u8,
        trailer: &Trailer,
        offsets: &[usize],
    ) -> IResult<&'a [u8], Plist> {
        let input = &data[offset..];
        let (input, counts) = if extra_info == 0xF {
            Self::parse_count(input)?
        } else {
            (input, extra_info as usize)
        };
        let (input, refs) = match trailer.object_ref_size {
            1 => count(map(be_u8, |v| v as usize), counts).parse(input)?,
            2 => count(map(be_u16, |v| v as usize), counts).parse(input)?,
            4 => count(map(be_u32, |v| v as usize), counts).parse(input)?,
            8 => count(map(be_u64, |v| v as usize), counts).parse(input)?,
            _ => panic!("Invalid object ref size"),
        };
        let mut array = Vec::with_capacity(counts);
        for object_ref_offset in refs {
            let (_, obj) = Self::parse_object(data, offsets[object_ref_offset], offsets, trailer)?;
            array.push(obj);
        }
        Ok((input, Plist::Array(array)))
    }
    fn parse_dict<'a>(
        data: &'a [u8],
        offset: usize,
        extra_info: u8,
        trailer: &Trailer,
        offsets: &[usize],
    ) -> IResult<&'a [u8], Plist> {
        let input = &data[offset..];
        let (input, counts) = if extra_info == 0xF {
            Self::parse_count(input)?
        } else {
            (input, extra_info as usize)
        };
        //先解析所有key refs
        let (input, key_refs) = match trailer.object_ref_size {
            1 => count(map(be_u8, |v| v as usize), counts).parse(input)?,
            2 => count(map(be_u16, |v| v as usize), counts).parse(input)?,
            4 => count(map(be_u32, |v| v as usize), counts).parse(input)?,
            8 => count(map(be_u64, |v| v as usize), counts).parse(input)?,
            _ => panic!("Invalid object ref size"),
        };
        let (input, value_refs) = match trailer.object_ref_size {
            1 => count(map(be_u8, |v| v as usize), counts).parse(input)?,
            2 => count(map(be_u16, |v| v as usize), counts).parse(input)?,
            4 => count(map(be_u32, |v| v as usize), counts).parse(input)?,
            8 => count(map(be_u64, |v| v as usize), counts).parse(input)?,
            _ => panic!("Invalid object ref size"),
        };
        let mut dict = vec![];
        let mut keys = vec![];
        for index in key_refs {
            let (_, key) = Self::parse_object(data, offsets[index], offsets, trailer)?;
            if let Plist::String(key) = key {
                keys.push(key);
            }
        }
        for (key_string, value_index) in keys.into_iter().zip(value_refs) {
            let new_offset = offsets[value_index];
            let (_, key) = Self::parse_object(data, new_offset, offsets, trailer)?;
            dict.push((key_string, key));
        }
        Ok((input, Plist::Dictionary(dict)))
    }
    fn parse_object<'a>(
        data: &'a [u8],
        offset: usize,
        offsets: &[usize],
        trailer: &Trailer,
    ) -> IResult<&'a [u8], Plist> {
        let input = &data[offset..];
        let (input, (object_type, extra_info)) = Self::parse_header(input)?;
        match object_type {
            0x0 => Self::parse_bool(input, extra_info),
            0x1 => Self::parse_integer(input, extra_info),
            0x2 => Self::parse_float(input, extra_info),
            0x3 => Self::parse_date(input, extra_info),
            0x4 => Self::parse_data(input, extra_info),
            0x5 => Self::parse_string(input, extra_info),
            0x6 => Self::parse_ascii_string(input, extra_info),
            0xA => Self::parse_array(data, offset + 1, extra_info, trailer, offsets),
            0xD => Self::parse_dict(data, offset + 1, extra_info, trailer, offsets),
            _ => Err(nom::Err::Error(nom::error::Error::new(
                input,
                nom::error::ErrorKind::Switch,
            ))),
        }
    }
}
