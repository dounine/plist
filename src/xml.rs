use crate::error::Error;
use crate::plist::Plist;
use chrono::{DateTime, Utc};
use nom::IResult;
use nom::Parser;
use nom::branch::alt;
use nom::bytes::complete::{is_not, tag, take_until};
use nom::character::complete::{char, digit1, multispace0};
use nom::combinator::{map, map_res, opt, recognize};
use nom::multi::many0;
use nom::sequence::{delimited, pair, terminated};
use std::collections::BTreeMap;

pub struct XmlPlist {}
impl XmlPlist {
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

    fn parse_dict(input: &str) -> IResult<&str, BTreeMap<String, Plist>> {
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
    fn parse_value(input: &str) -> IResult<&str, Plist> {
        let (input, _) = multispace0(input)?;
        if input.starts_with("<string>") {
            map(Self::parse_string, Plist::String).parse(input)
        } else if input.starts_with("<real>") {
            map(Self::parse_float, Plist::Float).parse(input)
        } else if input.starts_with("<date>") {
            map(Self::parse_date, Plist::Date).parse(input)
        } else if input.starts_with("<data>") {
            map(Self::parse_data, Plist::Data).parse(input)
        } else if input.starts_with("<integer>") {
            map(Self::parse_integer, Plist::Integer).parse(input)
        } else if input.starts_with("<true") || input.starts_with("<false") {
            map(Self::parse_boolean, Plist::Boolean).parse(input)
        } else if input.starts_with("<dict>") {
            map(Self::parse_dict, Plist::Dictionary).parse(input)
        } else {
            map(Self::parse_array, Plist::Array).parse(input)
        }
    }
    fn parse_array(input: &str) -> IResult<&str, Vec<Plist>> {
        let (input, _) = (multispace0, tag("<array>"), multispace0).parse(input)?;
        let (input, values) = many0(Self::parse_value).parse(input)?;
        let (input, _) = (multispace0, tag("</array>"), multispace0).parse(input)?;
        Ok((input, values))
    }
    pub fn parse(input: &[u8]) -> Result<Plist, Error> {
        let input = String::from_utf8_lossy(input).to_string();
        let input = input.as_str();
        let (input, _) = take_until("<plist")(input)?; //skip <?xml version="1.0" encoding="UTF-8"?>
        let (input, _) = terminated(is_not(">"), tag(">")).parse(input)?; //skip <plist ..>
        let (input, value) = map(Self::parse_dict, Plist::Dictionary).parse(input)?;
        let (_, _) = (multispace0, tag("</plist>"), multispace0).parse(input)?;
        Ok(value)
    }
}
