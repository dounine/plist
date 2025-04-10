use crate::error::Error;
use chrono::{DateTime, Utc};
use nom::branch::alt;
use nom::bytes::complete::{is_not, tag, take, take_until};
use nom::character::complete::{char, digit1, multispace0};
use nom::combinator::{map, map_res, opt, recognize};
use nom::multi::{count, many0};
use nom::number::complete::{be_f32, be_f64, be_u8, be_u16, be_u32, be_u64};
use nom::sequence::{delimited, pair, terminated};
use nom::{IResult, Parser};
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub enum PlistValue {
    Array(Vec<PlistValue>),
    Dictionary(BTreeMap<String, PlistValue>),
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
    Date(DateTime<Utc>),
    Data(Vec<u8>),
}
#[derive(Debug)]
struct Trailer {
    offset_table_offset_size: u8,
    object_ref_size: u8,
    num_objects: u64,
    top_object_offset: u64,
    offset_table_start: u64,
}
impl From<bool> for PlistValue {
    fn from(value: bool) -> Self {
        PlistValue::Boolean(value)
    }
}
impl From<i64> for PlistValue {
    fn from(value: i64) -> Self {
        PlistValue::Integer(value)
    }
}
impl From<&str> for PlistValue {
    fn from(value: &str) -> Self {
        PlistValue::String(value.to_string())
    }
}
impl From<String> for PlistValue {
    fn from(value: String) -> Self {
        PlistValue::String(value)
    }
}
#[allow(dead_code)]
impl PlistValue {
    pub fn to_xml(&self) -> String {
        let mut xml = String::from(
            r#"<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
"#,
        );

        xml.push_str(&self.convert_xml(0));
        xml.push_str("</plist>");
        xml
    }
    pub fn sort_key(&mut self) {
        if let PlistValue::Dictionary(dict) = self {
            let mut sorted_keys: Vec<String> = dict.keys().cloned().collect();
            sorted_keys.sort_by(|a, b| a.cmp(b));
            let mut sorted_dict = BTreeMap::new();
            for key in sorted_keys {
                if let Some(value) = dict.remove(&key) {
                    sorted_dict.insert(key, value);
                }
            }
            *dict = sorted_dict;
        }
    }
    fn convert_xml(&self, indent: usize) -> String {
        let indent_str = "\t".repeat(indent);
        let mut xml = String::new();
        match self {
            PlistValue::Float(value) => {
                xml.push_str(&format!("{}<real>{}</real>\n", indent_str, value))
            }
            PlistValue::Array(list) => {
                xml.push_str(&format!("{}<array>\n", indent_str));
                for item in list {
                    xml.push_str(&item.convert_xml(indent + 1));
                }
                xml.push_str(&format!("{}</array>\n", indent_str));
            }
            PlistValue::Dictionary(dict) => {
                xml.push_str(&format!("{}<dict>\n", indent_str));
                for (key, value) in dict {
                    xml.push_str(&format!("\t{}<key>{}</key>\n", indent_str, key));
                    xml.push_str(&value.convert_xml(indent + 1)); // 递归增加缩进
                }
                xml.push_str(&format!("{}</dict>\n", indent_str));
            }
            PlistValue::Boolean(value) => {
                if *value {
                    xml.push_str(&format!("{}<true/>\n", indent_str))
                } else {
                    xml.push_str(&format!("{}<false/>\n", indent_str))
                }
            }
            PlistValue::Integer(value) => {
                xml.push_str(&format!("{}<integer>{}</integer>\n", indent_str, value))
            }
            PlistValue::String(value) => {
                xml.push_str(&format!("{}<string>{}</string>\n", indent_str, value))
            }
            PlistValue::Date(value) => {
                xml.push_str(&format!("{}<date>{}</date>\n", indent_str, value))
            }
            PlistValue::Data(value) => {
                let value = String::from_utf8_lossy(value).to_string();
                xml.push_str(&format!("{}<data>{}</data>\n", indent_str, value))
            }
        }
        xml
    }
}
pub struct BPlist {}
impl BPlist {
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
    fn parse_integer(input: &[u8], extra_info: u8) -> IResult<&[u8], PlistValue> {
        let size = 1 << extra_info;
        match size {
            1 => map(be_u8, |v| PlistValue::Integer(v as i64)).parse(input),
            2 => map(be_u16, |v| PlistValue::Integer(v as i64)).parse(input),
            4 => map(be_u32, |v| PlistValue::Integer(v as i64)).parse(input),
            8 => map(be_u64, |v| PlistValue::Integer(v as i64)).parse(input),
            _ => Err(nom::Err::Error(nom::error::Error::new(
                input,
                nom::error::ErrorKind::Switch,
            ))),
        }
    }
    fn parse_string(input: &[u8], extra_info: u8) -> IResult<&[u8], PlistValue> {
        let len = extra_info;
        let (input, str_bytes) = take(len).parse(input)?;
        let str_value = String::from_utf8_lossy(str_bytes).to_string();
        Ok((input, PlistValue::String(str_value)))
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
    pub fn parse(input: &[u8]) -> IResult<&[u8], PlistValue> {
        let (_, _) = Self::parse_bplist_header(input)?;
        let (_, trailer) = Self::parse_trailer(&input[input.len() - 32..])?;
        let offset_table_start = trailer.offset_table_start as usize;
        let (_, offset_table) = Self::parse_offset_table(
            &input[offset_table_start..],
            trailer.num_objects,
            trailer.offset_table_offset_size,
        )?;
        let offset = offset_table[trailer.top_object_offset as usize];
        let object_data = &input[offset..];
        Self::parse_object(object_data, &offset_table, &trailer)
    }
    fn parse_float(input: &[u8], extra_info: u8) -> IResult<&[u8], PlistValue> {
        match extra_info {
            0 => map(be_f32, |v| PlistValue::Float(v as f64)).parse(input),
            2 => map(be_f32, |v| PlistValue::Float(v as f64)).parse(input),
            3 => map(be_f64, |v| PlistValue::Float(v as f64)).parse(input),
            _ => Err(nom::Err::Error(nom::error::Error::new(
                input,
                nom::error::ErrorKind::Switch,
            ))),
        }
    }
    fn parse_bool(input: &[u8], extra_info: u8) -> IResult<&[u8], PlistValue> {
        Ok((
            input,
            match extra_info {
                0x00 => PlistValue::Boolean(false),
                0x08 => PlistValue::Boolean(false),
                0x09 => PlistValue::Boolean(true),
                _ => {
                    return Err(nom::Err::Failure(nom::error::Error::new(
                        input,
                        nom::error::ErrorKind::TooLarge,
                    )));
                }
            },
        ))
    }
    fn parse_date(input: &[u8], _extra_info: u8) -> IResult<&[u8], PlistValue> {
        let (input, timestamp) = recognize(be_f64).parse(input)?;
        let bytes: [u8; 8] = timestamp.try_into().unwrap();
        let seconds_since_2001 = f64::from_be_bytes(bytes);
        let unix_timestamp = seconds_since_2001 + 978307200.0;

        // 5. 转换为 DateTime<Utc>
        let naive =
            DateTime::from_timestamp(unix_timestamp as i64, (unix_timestamp.fract() * 1e9) as u32)
                .unwrap();
        let datetime = DateTime::<Utc>::from(naive);
        Ok((input, PlistValue::Date(datetime)))
    }
    fn parse_data(input: &[u8], extra_info: u8) -> IResult<&[u8], PlistValue> {
        let (input, len) = if extra_info == 0xF {
            Self::parse_count(input)?
        } else {
            (input, extra_info as usize)
        };
        let (input, data) = take(len).parse(input)?;
        Ok((input, PlistValue::Data(data.to_vec())))
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
    fn parse_array<'a>(
        input: &'a [u8],
        extra_info: u8,
        trailer: &Trailer,
        offsets: &[usize],
    ) -> IResult<&'a [u8], PlistValue> {
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
        let mut input = input;
        for object_ref in refs {
            let obj: PlistValue;
            (input, obj) = Self::parse_object(input, offsets, trailer)?;
            array.push(obj);
        }
        Ok((input, PlistValue::Array(array)))
    }
    fn parse_dict<'a>(
        input: &'a [u8],
        extra_info: u8,
        trailer: &Trailer,
        offsets: &[usize],
    ) -> IResult<&'a [u8], PlistValue> {
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
        let (input, _) = match trailer.object_ref_size {
            1 => count(map(be_u8, |v| v as usize), counts).parse(input)?,
            2 => count(map(be_u16, |v| v as usize), counts).parse(input)?,
            4 => count(map(be_u32, |v| v as usize), counts).parse(input)?,
            8 => count(map(be_u64, |v| v as usize), counts).parse(input)?,
            _ => panic!("Invalid object ref size"),
        };
        let mut dict = BTreeMap::new();
        let mut key: PlistValue;
        let mut keys = vec![];
        let mut input = input;
        for _ in key_refs {
            (input, key) = Self::parse_object(input, offsets, trailer)?;
            if let PlistValue::String(key) = key {
                keys.push(key);
            }
        }
        for key_string in keys {
            (input, key) = Self::parse_object(input, offsets, trailer)?;
            dict.insert(key_string, key);
        }
        Ok((input, PlistValue::Dictionary(dict)))
    }
    fn parse_object<'a>(
        input: &'a [u8],
        offsets: &[usize],
        trailer: &Trailer,
    ) -> IResult<&'a [u8], PlistValue> {
        let (object_data, (object_type, extra_info)) = Self::parse_header(input)?;
        match object_type {
            0x0 => Self::parse_bool(object_data, extra_info),
            0x1 => Self::parse_integer(object_data, extra_info),
            0x2 => Self::parse_float(object_data, extra_info),
            0x3 => Self::parse_date(object_data, extra_info),
            0x5 => Self::parse_string(object_data, extra_info),
            0x6 => Self::parse_data(object_data, extra_info),
            0xA => Self::parse_array(object_data, extra_info, trailer, offsets),
            0xD => Self::parse_dict(object_data, extra_info, trailer, offsets),
            _ => Err(nom::Err::Error(nom::error::Error::new(
                object_data,
                nom::error::ErrorKind::Switch,
            ))),
        }
    }
}
impl PlistValue {
    fn parse_key(input: &str) -> IResult<&str, &str> {
        let (input, _) = multispace0(input)?;
        delimited(tag("<key>"), take_until("<"), tag("</key>")).parse(input)
    }
    fn parse_string(input: &str) -> IResult<&str, String> {
        delimited(tag("<string>"), take_until("<"), tag("</string>"))
            .parse(input)
            .map(|(next_input, result)| (next_input, result.to_string()))
    }
    fn parse_float(input: &str) -> IResult<&str, f64> {
        delimited(tag("<real>"), take_until("<"), tag("</real>"))
            .parse(input)
            .map(|(next_input, result)| (next_input, result.parse().unwrap()))
    }
    fn parse_date(input: &str) -> IResult<&str, DateTime<Utc>> {
        delimited(tag("<date>"), take_until("<"), tag("</date>"))
            .parse(input)
            .map(|(next_input, result)| {
                (
                    next_input,
                    DateTime::parse_from_rfc3339(result).unwrap().into(),
                )
            })
    }
    fn parse_data(input: &str) -> IResult<&str, Vec<u8>> {
        delimited(tag("<data>"), take_until("<"), tag("</data>"))
            .parse(input)
            .map(|(next_input, result)| (next_input, result.as_bytes().to_vec()))
    }
    fn parse_integer(input: &str) -> IResult<&str, i64> {
        let (input, _) = multispace0(input)?;
        let (input, result) = map_res(
            delimited(
                tag("<integer>"),
                recognize(pair(opt(alt((char('-'), char('+')))), digit1)),
                // recognize(preceded(opt(char('-')), digit1)),
                tag("</integer>"),
            ),
            |s: &str| s.parse(),
        )
        .parse(input)?;
        Ok((input, result))
    }
    fn parse_boolean(input: &str) -> IResult<&str, bool> {
        let (input, _) = multispace0(input)?;
        alt((
            map(tag("<true/>"), |_| true),
            map(tag("<false/>"), |_| false),
        ))
        .parse(input)
    }

    fn parse_dict(input: &str) -> IResult<&str, BTreeMap<String, Self>> {
        let (input, _) = multispace0(input)?;
        let (input, _) = tag("<dict>")(input)?;
        let (input, values) = many0((Self::parse_key, Self::parse_value)).parse(input)?;
        let mut dict = BTreeMap::new();
        for (key, value) in values {
            dict.insert(key.to_string(), value);
        }
        let (input, _) = multispace0(input)?;
        let (input, _) = tag("</dict>")(input)?;
        Ok((input, dict))
    }
    fn parse_value(input: &str) -> IResult<&str, Self> {
        let (input, _) = multispace0(input)?;
        if input.starts_with("<string>") {
            map(Self::parse_string, Self::String).parse(input)
        } else if input.starts_with("<real>") {
            map(Self::parse_float, Self::Float).parse(input)
        } else if input.starts_with("<date>") {
            map(Self::parse_date, Self::Date).parse(input)
        } else if input.starts_with("<data>") {
            map(Self::parse_data, Self::Data).parse(input)
        } else if input.starts_with("<integer>") {
            map(Self::parse_integer, Self::Integer).parse(input)
        } else if input.starts_with("<true") || input.starts_with("<false") {
            map(Self::parse_boolean, Self::Boolean).parse(input)
        } else if input.starts_with("<dict>") {
            map(Self::parse_dict, Self::Dictionary).parse(input)
        } else {
            map(Self::parse_array, Self::Array).parse(input)
        }
    }
    fn parse_array(input: &str) -> IResult<&str, Vec<Self>> {
        let (input, _) = (multispace0, tag("<array>"), multispace0).parse(input)?;
        let (input, values) = many0(Self::parse_value).parse(input)?;
        let (input, _) = (multispace0, tag("</array>"), multispace0).parse(input)?;
        Ok((input, values))
    }
    pub fn parse(input: &[u8]) -> Result<Self, Error> {
        //判断是不是bplist00
        if input.starts_with(b"bplist00") {
            let (_, value) = BPlist::parse(input).map_err(|e| Error::Error(e.to_string()))?;
            Ok(value)
        } else {
            let input = String::from_utf8_lossy(input).to_string();
            let input = input.as_str();
            let (input, _) = take_until("<plist")(input)?; //skip <?xml version="1.0" encoding="UTF-8"?>
            let (input, _) = terminated(is_not(">"), tag(">")).parse(input)?; //skip <plist ..>
            let (input, value) = map(Self::parse_dict, Self::Dictionary).parse(input)?;
            let (_, _) = (multispace0, tag("</plist>"), multispace0).parse(input)?;
            Ok(value)
        }
    }
}
#[cfg(test)]
mod bplist_test {
    use crate::plist::BPlist;
    use std::fs;

    #[test]
    fn test_parse_binary() {
        let data = fs::read("./data/InfoPlist.strings").unwrap();
        let (input, plist) = BPlist::parse(&data).unwrap();
        println!("{:?}", plist)
    }
}
#[cfg(test)]
mod plist_test {
    use crate::plist::PlistValue;

    #[test]
    fn test_parse() {
        let xml = r#"<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>AppIDName</key>
	<string>ipadump</string>
	<key>ApplicationIdentifierPrefix</key>
	<array>
	<string>Q4J8HDK83K</string>
	</array>
	<key>CreationDate</key>
	<date>2024-08-17T02:24:50Z</date>
	<key>Platform</key>
	<array>
		<string>iOS</string>
		<string>xrOS</string>
		<string>visionOS</string>
	</array>
	<key>IsXcodeManaged</key>
	<false/>
	<key>DeveloperCertificates</key>
	<array>
		<data>MIIFyTCCBLGgAwIBAgIQQ1PQwRY7PCEtVwuLsjLWtTANBgkqhkiG9w0BAQsFADB1MUQwQgYDVQQDDDtBcHBsZSBXb3JsZHdpZGUgRGV2ZWxvcGVyIFJlbGF0aW9ucyBDZXJ0aWZpY2F0aW9uIEF1dGhvcml0eTELMAkGA1UECwwCRzMxEzARBgNVBAoMCkFwcGxlIEluYy4xCzAJBgNVBAYTAlVTMB4XDTI0MDgxNzAyMTEyMVoXDTI1MDgxNzAyMTEyMFowgY8xGjAYBgoJkiaJk/IsZAEBDApRNEo4SERLODNLMTcwNQYDVQQDDC5BcHBsZSBEaXN0cmlidXRpb246IEh1YW5MYWkgaHVhbmcgKFE0SjhIREs4M0spMRMwEQYDVQQLDApRNEo4SERLODNLMRYwFAYDVQQKDA1IdWFuTGFpIGh1YW5nMQswCQYDVQQGEwJDTjCCASIwDQYJKoZIhvcNAQEBBQADggEPADCCAQoCggEBAOhgAnWvOEZAjxkFYetRAnR6Bw/yKotTXcDSLvi+rtgU81rqiImgpVsyhiVROxbAe7x2KOXg3PaVrgX+Df5VxaBIqZqUJb81BHEviszpAbAXutTU3az2YUn/DqJRxy13sXWedkgFoJbIQ8x22Ia0pBogaa8MQFyEPVMelHzBD/vTpORhG1C2bDCcio4JFvk3D/KfDuVW4mNbgg6yroiNns2xSbODzcD7zu4huHpUgUKlAfc1agI0g2UjcRen8uBn1KzItUgYXmW43CKM+Bt8Uz0Ds1TmEOU2nXMwlw3qM13xYP0YKC8DovZbSReb7xDet/5nMzo/yGUHmWsGOfiumxcCAwEAAaOCAjgwggI0MAwGA1UdEwEB/wQCMAAwHwYDVR0jBBgwFoAUCf7AFZD5r2QKkhK5JihjDJfsp7IwcAYIKwYBBQUHAQEEZDBiMC0GCCsGAQUFBzAChiFodHRwOi8vY2VydHMuYXBwbGUuY29tL3d3ZHJnMy5kZXIwMQYIKwYBBQUHMAGGJWh0dHA6Ly9vY3NwLmFwcGxlLmNvbS9vY3NwMDMtd3dkcmczMDUwggEeBgNVHSAEggEVMIIBETCCAQ0GCSqGSIb3Y2QFATCB/zCBwwYIKwYBBQUHAgIwgbYMgbNSZWxpYW5jZSBvbiB0aGlzIGNlcnRpZmljYXRlIGJ5IGFueSBwYXJ0eSBhc3N1bWVzIGFjY2VwdGFuY2Ugb2YgdGhlIHRoZW4gYXBwbGljYWJsZSBzdGFuZGFyZCB0ZXJtcyBhbmQgY29uZGl0aW9ucyBvZiB1c2UsIGNlcnRpZmljYXRlIHBvbGljeSBhbmQgY2VydGlmaWNhdGlvbiBwcmFjdGljZSBzdGF0ZW1lbnRzLjA3BggrBgEFBQcCARYraHR0cHM6Ly93d3cuYXBwbGUuY29tL2NlcnRpZmljYXRlYXV0aG9yaXR5LzAWBgNVHSUBAf8EDDAKBggrBgEFBQcDAzAdBgNVHQ4EFgQUTNc65ckP8Lt59YSojGJKJAFYR/EwDgYDVR0PAQH/BAQDAgeAMBMGCiqGSIb3Y2QGAQcBAf8EAgUAMBMGCiqGSIb3Y2QGAQQBAf8EAgUAMA0GCSqGSIb3DQEBCwUAA4IBAQCV7+yY3wHpUnaJvAlF+LBAO6RIRFtFWhIWA6Xof52AVNvWQnjPg03/cUM3Lc3HCq17Agd/l2vF7BorfJobZzZkOcdqfmSbbUAIF0bv3XH41xB0GNqAPuQG5i+TMssMDzlv/O7tIWhh9sN6y6vtiqmiG6OABeO/JxN71GmNgCbjPQGeTspGagMmKns70iMAbUhxcZxtXSkl7UCv+AZxQ/AXKAGaS7L/+js5cRjKNpaepHrPMF7YO0CmP1BcG2GisbcVxfDOsFUVGKVbAzIgb58/JcPei828Ue09a3XcxrBTCTcGtJCx73IlxOv1ldTrkY+jNU99TOowGu7PuOqiGYWm</data>
	</array>

	<key>DER-Encoded-Profile</key>
	<data>MIIPmQYJKoZIhvcNAQcCoIIPijCCD4YCAQExDzANBglghkgBZQMEAgEFADCCBVMGCSqGSIb3DQEHAaCCBUQEggVAMYIFPDAMDAdWZXJzaW9uAgEBMBAMClRpbWVUb0xpdmUCAgFsMBMMBE5hbWUMC2lwYWR1bXAuY29tMBMMDklzWGNvZGVNYW5hZ2VkAQEAMBQMCUFwcElETmFtZQwHaXBhZHVtcDAZDAhUZWFtTmFtZQwNSHVhbkxhaSBodWFuZzAdDAxDcmVhdGlvbkRhdGUXDTI0MDgxNzAyMjQ1MFowHgwOVGVhbUlkZW50aWZpZXIwDAwKUTRKOEhESzgzSzAfDA5FeHBpcmF0aW9uRGF0ZRcNMjUwODE3MDIxMTIwWjAgDBdQcm9maWxlRGlzdHJpYnV0aW9uVHlwZQwFQURIT0MwIQwIUGxhdGZvcm0wFQwDaU9TDAR4ck9TDAh2aXNpb25PUzArDBtBcHBsaWNhdGlvbklkZW50aWZpZXJQcmVmaXgwDAwKUTRKOEhESzgzSzAsDARVVUlEDCRkZTExYWRkOS0xNzI2LTQxZGQtYTc2Mi00NTdmMTljOTdhYTIwOwwVRGV2ZWxvcGVyQ2VydGlmaWNhdGVzMCIEIMvjeTwnoeCbjLwvAV4XgH6c/8trlmmna9zejC8+jqyBMIIBXAwSUHJvdmlzaW9uZWREZXZpY2VzMIIBRAwZMDAwMDgxMDEtMDAwOTE1NDAzNDQyMDAxRQwZMDAwMDgxMjAtMDAxNDE1OEUzRTk4MjAxRQwZMDAwMDgxMDMtMDAwRDY1RUExRUQwQzAxRQwZMDAwMDgxMTAtMDAwNjE4REEzQ0MyODAxRQwZMDAwMDgxMTItMDAwNjUwNTQzNDUzQTAxRQwZMDAwMDgxMjAtMDAwQTIxNEMzQUUyMjAxRQwZMDAwMDgwMjAtMDAxQzRENTQyMUYxMDAyRQwZMDAwMDgxMzAtMDAxQTM0MzEzNjYyMDAxQwwZMDAwMDgxMTAtMDAwNjJEMzYzRTkyODAxRQwZMDAwMDgxMDEtMDAwQTQwRUExRTIyMDAxRQwZMDAwMDgxMjAtMDAxNjA0NTkwRTlCQzAxRQwZMDAwMDgwMzAtMDAxRDI0MjIzRTIwODAyRTCCAiAMDEVudGl0bGVtZW50c3CCAg4CAQGwggIHMCwMFmFwcGxpY2F0aW9uLWlkZW50aWZpZXIMElE0SjhIREs4M0suaXBhZHVtcDAdDA9hcHMtZW52aXJvbm1lbnQMCnByb2R1Y3Rpb24wKwwmY29tLmFwcGxlLmRldmVsb3Blci5hc3NvY2lhdGVkLWRvbWFpbnMMASowIgwdY29tLmFwcGxlLmRldmVsb3Blci5oZWFsdGhraXQBAf8wOAwkY29tLmFwcGxlLmRldmVsb3Blci5oZWFsdGhraXQuYWNjZXNzMBAMDmhlYWx0aC1yZWNvcmRzMDYMMWNvbS5hcHBsZS5kZXZlbG9wZXIuaGVhbHRoa2l0LmJhY2tncm91bmQtZGVsaXZlcnkBAf8wOAwzY29tLmFwcGxlLmRldmVsb3Blci5oZWFsdGhraXQucmVjYWxpYnJhdGUtZXN0aW1hdGVzAQH/MDEMI2NvbS5hcHBsZS5kZXZlbG9wZXIudGVhbS1pZGVudGlmaWVyDApRNEo4SERLODNLMDgMM2NvbS5hcHBsZS5kZXZlbG9wZXIudXNlcm5vdGlmaWNhdGlvbnMuY29tbXVuaWNhdGlvbgEB/zATDA5nZXQtdGFzay1hbGxvdwEBADA5DBZrZXljaGFpbi1hY2Nlc3MtZ3JvdXBzMB8MDFE0SjhIREs4M0suKgwPY29tLmFwcGxlLnRva2VuoIIIPDCCAkMwggHJoAMCAQICCC3F/IjSxUuVMAoGCCqGSM49BAMDMGcxGzAZBgNVBAMMEkFwcGxlIFJvb3QgQ0EgLSBHMzEmMCQGA1UECwwdQXBwbGUgQ2VydGlmaWNhdGlvbiBBdXRob3JpdHkxEzARBgNVBAoMCkFwcGxlIEluYy4xCzAJBgNVBAYTAlVTMB4XDTE0MDQzMDE4MTkwNloXDTM5MDQzMDE4MTkwNlowZzEbMBkGA1UEAwwSQXBwbGUgUm9vdCBDQSAtIEczMSYwJAYDVQQLDB1BcHBsZSBDZXJ0aWZpY2F0aW9uIEF1dGhvcml0eTETMBEGA1UECgwKQXBwbGUgSW5jLjELMAkGA1UEBhMCVVMwdjAQBgcqhkjOPQIBBgUrgQQAIgNiAASY6S89QHKk7ZMicoETHN0QlfHFo05x3BQW2Q7lpgUqd2R7X04407scRLV/9R+2MmJdyemEW08wTxFaAP1YWAyl9Q8sTQdHE3Xal5eXbzFc7SudeyA7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</data>

	<key>Entitlements</key>
	<dict>

				<key>com.apple.developer.associated-domains</key>
		<string>*</string>

				<key>com.apple.developer.healthkit.recalibrate-estimates</key>
		<true/>

				<key>application-identifier</key>
		<string>Q4J8HDK83K.ipadump</string>

				<key>keychain-access-groups</key>
		<array>
				<string>Q4J8HDK83K.*</string>
				<string>com.apple.token</string>
		</array>

				<key>com.apple.developer.healthkit</key>
		<true/>

				<key>com.apple.developer.healthkit.access</key>
		<array>
				<string>health-records</string>
		</array>

				<key>get-task-allow</key>
		<false/>

				<key>com.apple.developer.team-identifier</key>
		<string>Q4J8HDK83K</string>

				<key>com.apple.developer.usernotifications.communication</key>
		<true/>

				<key>com.apple.developer.healthkit.background-delivery</key>
		<true/>

				<key>aps-environment</key>
		<string>production</string>

	</dict>
	<key>ExpirationDate</key>
	<date>2025-08-17T02:11:20Z</date>
	<key>Name</key>
	<string>ipadump.com</string>
	<key>ProvisionedDevices</key>
	<array>
		<string>00008101-000915403442001E</string>
		<string>00008120-0014158E3E98201E</string>
		<string>00008103-000D65EA1ED0C01E</string>
		<string>00008110-000618DA3CC2801E</string>
		<string>00008112-000650543453A01E</string>
		<string>00008120-000A214C3AE2201E</string>
		<string>00008020-001C4D5421F1002E</string>
		<string>00008130-001A34313662001C</string>
		<string>00008110-00062D363E92801E</string>
		<string>00008101-000A40EA1E22001E</string>
		<string>00008120-001604590E9BC01E</string>
		<string>00008030-001D24223E20802E</string>
	</array>
	<key>TeamIdentifier</key>
	<array>
		<string>Q4J8HDK83K</string>
	</array>
	<key>TeamName</key>
	<string>HuanLai huang</string>
	<key>TimeToLive</key>
	<integer>364</integer>
	<key>UUID</key>
	<string>de11add9-1726-41dd-a762-457f19c97aa2</string>
	<key>Version</key>
	<integer>1</integer>
</dict>
</plist>
    "#;
        let mut value = PlistValue::parse(xml.as_bytes()).unwrap();
        if let PlistValue::Dictionary(dict) = &mut value {
            if let Some(PlistValue::Boolean(value)) = dict.get("hello") {
                assert_eq!(*value, true);
            }
        }
        // value.sort_key();
        println!("{}", value.to_xml());
    }
}
