use std::str::FromStr;

use nom::{
    branch::alt,
    bytes::complete::{escaped_transform, is_not, tag, take_while, take_while_m_n},
    character::complete::{alphanumeric1, char, digit1},
    combinator::{self, map, map_opt, map_res},
    number::complete::double,
    sequence::{delimited, preceded, separated_pair},
    IResult,
};

pub enum Command {
    Read(ReadCommand),
    Write(WriteCommand),
}

impl FromStr for Command {
    type Err = ();

    // This is the entrypoint for this module
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let Ok((_, command)) = alt((read_command, write_command))(input) else {
            return Err(());
        };
        Ok(command)
    }
}

pub struct ReadCommand {
    name: String,
    object: ObjectIndex,
    data_type: CoeType,
}

impl ReadCommand {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn object(&self) -> (u16, u8) {
        (self.object.address, self.object.sub_index)
    }

    pub fn format(&self, value: &[u8]) -> Result<String, ()> {
        Ok(match self.data_type {
            CoeType::Bool => format!("{}", bool_try_from_le_bytes(value)?),
            CoeType::Uint8 => format!("{}", u8_try_from_le_bytes(value)?),
            CoeType::Uint16 => format!("{}", u16_try_from_le_bytes(value)?),
            CoeType::Uint32 => format!("{}", u32_try_from_le_bytes(value)?),
            CoeType::Uint64 => format!("{}", u64_try_from_le_bytes(value)?),
            CoeType::Int8 => format!("{}", i8_try_from_le_bytes(value)?),
            CoeType::Int16 => format!("{}", i16_try_from_le_bytes(value)?),
            CoeType::Int32 => format!("{}", i32_try_from_le_bytes(value)?),
            CoeType::Int64 => format!("{}", i64_try_from_le_bytes(value)?),
            CoeType::ArrayUint8 => format!("{:?}", arr_u8_try_from_le_bytes(value)?),
            CoeType::ArrayUint16 => format!("{:?}", arr_u8_try_from_le_bytes(value)?),
            CoeType::ArrayUint32 => format!("{:?}", arr_u8_try_from_le_bytes(value)?),
            CoeType::ArrayUint64 => format!("{:?}", arr_u8_try_from_le_bytes(value)?),
            CoeType::ArrayInt8 => format!("{:?}", arr_u8_try_from_le_bytes(value)?),
            CoeType::ArrayInt16 => format!("{:?}", arr_u8_try_from_le_bytes(value)?),
            CoeType::ArrayInt32 => format!("{:?}", arr_u8_try_from_le_bytes(value)?),
            CoeType::ArrayInt64 => format!("{:?}", arr_u8_try_from_le_bytes(value)?),
            CoeType::Float32 => format!("{}", f32_try_from_le_bytes(value)?),
            CoeType::Float64 => format!("{}", f64_try_from_le_bytes(value)?),
            CoeType::String => format!("{}", string_try_from_bytes(value)?),
        })
    }
}

fn bool_try_from_le_bytes(bytes: &[u8]) -> Result<bool, ()> {
    // Per CiA 301 §7.1.4.3, 0 is falsey.  It doesn't specify what the
    // size of a BOOLEAN is.  It also says that 1 is truthy; though I'll
    // handle other values as true as well.
    Ok(u8_try_from_le_bytes(bytes)? != 0)
}

fn u8_try_from_le_bytes(bytes: &[u8]) -> Result<u8, ()> {
    if let Some(bytes) = bytes.first_chunk::<1>() {
        return Ok(u8::from_le_bytes(*bytes));
    }
    Err(())
}

fn u16_try_from_le_bytes(bytes: &[u8]) -> Result<u16, ()> {
    if let Some(bytes) = bytes.first_chunk::<2>() {
        return Ok(u16::from_le_bytes(*bytes));
    }
    Err(())
}

fn u32_try_from_le_bytes(bytes: &[u8]) -> Result<u32, ()> {
    if let Some(bytes) = bytes.first_chunk::<4>() {
        return Ok(u32::from_le_bytes(*bytes));
    }
    Err(())
}

fn u64_try_from_le_bytes(bytes: &[u8]) -> Result<u64, ()> {
    if let Some(bytes) = bytes.first_chunk::<8>() {
        return Ok(u64::from_le_bytes(*bytes));
    }
    Err(())
}

fn i8_try_from_le_bytes(bytes: &[u8]) -> Result<i8, ()> {
    if let Some(bytes) = bytes.first_chunk::<1>() {
        return Ok(i8::from_le_bytes(*bytes));
    }
    Err(())
}

fn i16_try_from_le_bytes(bytes: &[u8]) -> Result<i16, ()> {
    if let Some(bytes) = bytes.first_chunk::<2>() {
        return Ok(i16::from_le_bytes(*bytes));
    }
    Err(())
}

fn i32_try_from_le_bytes(bytes: &[u8]) -> Result<i32, ()> {
    if let Some(bytes) = bytes.first_chunk::<4>() {
        return Ok(i32::from_le_bytes(*bytes));
    }
    Err(())
}

fn i64_try_from_le_bytes(bytes: &[u8]) -> Result<i64, ()> {
    if let Some(bytes) = bytes.first_chunk::<8>() {
        return Ok(i64::from_le_bytes(*bytes));
    }
    Err(())
}

fn arr_u8_try_from_le_bytes(bytes: &[u8]) -> Result<&[u8], ()> {
    Ok(bytes)
}

fn arr_u16_try_from_le_bytes(bytes: &[u8]) -> Result<&[u16], ()> {
    if bytes.len() % 2 != 0 {
        return Err(());
    }
    // so presumably checking the length of the slice makes this safe?  Right??
    unsafe { Ok(bytes.align_to::<u16>().1) }
}

fn arr_u32_try_from_le_bytes(bytes: &[u8]) -> Result<&[u32], ()> {
    if bytes.len() % 4 != 0 {
        return Err(());
    }
    unsafe { Ok(bytes.align_to::<u32>().1) }
}

fn arr_u64_try_from_le_bytes(bytes: &[u8]) -> Result<&[u64], ()> {
    if bytes.len() % 8 != 0 {
        return Err(());
    }
    unsafe { Ok(bytes.align_to::<u64>().1) }
}

fn arr_i8_try_from_le_bytes(bytes: &[u8]) -> Result<&[i8], ()> {
    unsafe { Ok(bytes.align_to::<i8>().1) }
}

fn arr_i16_try_from_le_bytes(bytes: &[u8]) -> Result<&[i16], ()> {
    if bytes.len() % 2 != 0 {
        return Err(());
    }
    unsafe { Ok(bytes.align_to::<i16>().1) }
}

fn arr_i32_try_from_le_bytes(bytes: &[u8]) -> Result<&[i32], ()> {
    if bytes.len() % 4 != 0 {
        return Err(());
    }
    unsafe { Ok(bytes.align_to::<i32>().1) }
}

fn arr_i64_try_from_le_bytes(bytes: &[u8]) -> Result<&[i64], ()> {
    if bytes.len() % 8 != 0 {
        return Err(());
    }
    unsafe { Ok(bytes.align_to::<i64>().1) }
}

fn f32_try_from_le_bytes(bytes: &[u8]) -> Result<f32, ()> {
    if let Some(bytes) = bytes.first_chunk::<4>() {
        return Ok(f32::from_le_bytes(*bytes));
    }
    Err(())
}

fn f64_try_from_le_bytes(bytes: &[u8]) -> Result<f64, ()> {
    if let Some(bytes) = bytes.first_chunk::<8>() {
        return Ok(f64::from_le_bytes(*bytes));
    }
    Err(())
}

fn string_try_from_bytes(bytes: &[u8]) -> Result<String, ()> {
    // per CiA 301 §7.1.6.3, VISIBLE_STRINGs are ISO 646-1973 compliant,
    // i.e. ASCII strings.  §7.1.6.4 suggests that unicode strings are
    // possible, but it doesn't say what the actual encoding should be.
    // I'll assume UTF-8 and hope for the best.  It's probably
    // manufacturer-dependent fuckery.
    String::from_utf8(bytes.into()).map_err(|_| ())
}

pub struct WriteCommand {
    name: String,
    object: ObjectIndex,
    value: Value,
}

impl WriteCommand {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn object(&self) -> (u16, u8) {
        (self.object.address, self.object.sub_index)
    }

    pub fn to_le_bytes(&self) -> Vec<u8> {
        self.value.clone().to_bytes()
    }
}

// Read from address
// <read> ::= 'r ' <name> ' ' <object_index> ' ' <data_type>
// r 0x1008:0 String
// => "EK1100"
fn read_command(input: &str) -> IResult<&str, Command> {
    map(
        preceded(
            tag("r "),
            separated_pair(
                name,
                char(' '),
                separated_pair(object_index, char(' '), data_type),
            ),
        ),
        |(name, (object, data_type))| {
            Command::Read(ReadCommand {
                name: name.into(),
                object,
                data_type,
            })
        },
    )(input)
}

fn name(input: &str) -> IResult<&str, &str> {
    alphanumeric1(input)
}

// Write to address
// <write> ::= 'w ' <object_index> ' ' <value>
// w 0x1a00:0 0 u8
fn write_command(input: &str) -> IResult<&str, Command> {
    map(
        preceded(
            tag("w "),
            separated_pair(
                name,
                char(' '),
                separated_pair(object_index, char(' '), value),
            ),
        ),
        |(name, (object, value))| {
            Command::Write(WriteCommand {
                name: name.into(),
                object,
                value,
            })
        },
    )(input)
}

#[derive(Clone)]
enum Value {
    Int8(i8),
    Int16(i16),
    Int32(i32),
    Int64(i64),
    Float(f64),
    String(String),
}

impl Value {
    fn to_bytes<'a>(self) -> Vec<u8> {
        match self {
            Value::Int8(i) => i.to_le_bytes().into(), // CiA 301 §7.1.1, CoE uses little-endian numbers
            Value::Int16(i) => i.to_le_bytes().into(),
            Value::Int32(i) => i.to_le_bytes().into(),
            Value::Int64(i) => i.to_le_bytes().into(),
            Value::Float(f) => f.to_le_bytes().into(),
            Value::String(s) => s.as_bytes().into(),
        }
    }
}

// <value> ::= <string> | <number>
fn value(input: &str) -> IResult<&str, Value> {
    alt((string, number))(input)
}

// <string> ::= '"' (characters with backslash-escaped double-quotes) '"'
fn string(input: &str) -> IResult<&str, Value> {
    map(
        delimited(
            char('"'),
            escaped_transform(
                is_not("\\"),
                '\\',
                alt((
                    combinator::value("\\", tag("\\")),
                    combinator::value("\"", tag("\"")),
                    combinator::value("\n", tag("n")),
                    combinator::value("\t", tag("t")),
                    combinator::value("\r", tag("r")),
                )),
            ),
            char('"'),
        ),
        |s| Value::String(s),
    )(input)
}

// <number> ::= <int> | <float>
fn number(input: &str) -> IResult<&str, Value> {
    alt((int, map(double, Value::Float)))(input)
}

// <int> ::= <hex> | <decimal>
fn int(input: &str) -> IResult<&str, Value> {
    let (input, int) = alt((hex, decimal))(input)?;
    alt((
        combinator::value(Value::Int8(int.try_into().unwrap_or(i8::MAX)), tag("i8")),
        combinator::value(Value::Int16(int.try_into().unwrap_or(i16::MAX)), tag("i16")),
        combinator::value(Value::Int32(int.try_into().unwrap_or(i32::MAX)), tag("i32")),
        combinator::value(Value::Int64(int.try_into().unwrap_or(i64::MAX)), tag("i64")),
    ))(input)
}

// <hex> ::= '0x' <hex_digit>+
fn hex(input: &str) -> IResult<&str, i64> {
    map_res(
        preceded(tag("0x"), take_while_m_n(0, 16, |c: char| c.is_digit(16))),
        i64::from_str,
    )(input)
}

// <decimal> ::= <decimal_digit>+
fn decimal(input: &str) -> IResult<&str, i64> {
    map_res(take_while(|c: char| c.is_digit(10)), i64::from_str)(input)
}

struct ObjectIndex {
    address: u16,
    sub_index: u8,
}

// <object_index> ::= <address> ':' <sub_index>
fn object_index(input: &str) -> IResult<&str, ObjectIndex> {
    map(
        separated_pair(address, char(':'), sub_index),
        |(address, sub_index)| ObjectIndex { address, sub_index },
    )(input)
}
// <address> ::= '0x' <hex_digit>{4}
fn address(input: &str) -> IResult<&str, u16> {
    map_res(
        preceded(tag("0x"), take_while(|c: char| c.is_digit(16))),
        u16::from_str,
    )(input)
}
// <sub_index> ::= <decimal_digit>{,3}
fn sub_index(input: &str) -> IResult<&str, u8> {
    map_res(digit1, u8::from_str)(input)
}

enum CoeType {
    Bool,
    Uint8,
    Uint16,
    Uint32,
    Uint64,
    Int8,
    Int16,
    Int32,
    Int64,
    ArrayUint8,
    ArrayUint16,
    ArrayUint32,
    ArrayUint64,
    ArrayInt8,
    ArrayInt16,
    ArrayInt32,
    ArrayInt64,
    Float32,
    Float64,
    String,
}

impl FromStr for CoeType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "bool" => Ok(CoeType::Bool),
            "u8" => Ok(CoeType::Uint8),
            "u16" => Ok(CoeType::Uint16),
            "u32" => Ok(CoeType::Uint32),
            "u64" => Ok(CoeType::Uint64),
            "i8" => Ok(CoeType::Int8),
            "i16" => Ok(CoeType::Int16),
            "i32" => Ok(CoeType::Int32),
            "i64" => Ok(CoeType::Int64),
            "[u8]" => Ok(CoeType::ArrayUint8),
            "[u16]" => Ok(CoeType::ArrayUint16),
            "[u32]" => Ok(CoeType::ArrayUint32),
            "[u64]" => Ok(CoeType::ArrayUint64),
            "[i8]" => Ok(CoeType::ArrayInt8),
            "[i16]" => Ok(CoeType::ArrayInt16),
            "[i32]" => Ok(CoeType::ArrayInt32),
            "[i64]" => Ok(CoeType::ArrayInt64),
            "f32" => Ok(CoeType::Float32),
            "f64" => Ok(CoeType::Float64),
            "String" => Ok(CoeType::String),
            _ => Err(()),
        }
    }
}

// <data_type> ::= <bool_type> | <int_type> | <int_array_type> | <float_type> | <string_type>
fn data_type(input: &str) -> IResult<&str, CoeType> {
    alt((bool_type, int_type, int_array_type, float_type, string_type))(input)
}

// <bool_type> ::= 'bool'
fn bool_type(input: &str) -> IResult<&str, CoeType> {
    map(tag("bool"), |_| CoeType::Bool)(input)
}

// <int_type> ::= 'u8' | 'u16' | 'u32' | 'u64' | 'i8' | 'i16' | 'i32' | 'i64'
fn int_type(input: &str) -> IResult<&str, CoeType> {
    map_res(
        alt((
            tag("u8"),
            tag("u16"),
            tag("u32"),
            tag("u64"),
            tag("i8"),
            tag("i16"),
            tag("i32"),
            tag("i64"),
        )),
        CoeType::from_str,
    )(input)
}

// <int_array_type> ::= '[' <int_type> ']'
fn int_array_type(input: &str) -> IResult<&str, CoeType> {
    map_opt(delimited(char('['), int_type, char(']')), |t| match t {
        CoeType::Uint8 => Some(CoeType::ArrayUint8),
        CoeType::Uint16 => Some(CoeType::ArrayUint16),
        CoeType::Uint32 => Some(CoeType::ArrayUint32),
        CoeType::Uint64 => Some(CoeType::ArrayUint64),
        CoeType::Int8 => Some(CoeType::ArrayInt8),
        CoeType::Int16 => Some(CoeType::ArrayInt16),
        CoeType::Int32 => Some(CoeType::ArrayInt32),
        CoeType::Int64 => Some(CoeType::ArrayInt64),
        _ => None,
    })(input)
}

// <float_type> ::= 'f32' | 'f64'
fn float_type(input: &str) -> IResult<&str, CoeType> {
    map_res(alt((tag("f32"), tag("f64"))), CoeType::from_str)(input)
}

// <string_type> ::= 'String'
fn string_type(input: &str) -> IResult<&str, CoeType> {
    map(tag("String"), |_| CoeType::String)(input)
}
