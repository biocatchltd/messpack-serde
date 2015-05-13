use std::io;
use std::io::{Read, Cursor};
use std::result::Result;
use std::str::{from_utf8, Utf8Error};

use byteorder;
use byteorder::ReadBytesExt;

use super::super::Marker;

/// Represents an error that can occur when attempting to read bytes from the reader.
///
/// This is a thin wrapper over the standard `io::Error` type. Namely, it adds one additional error
/// case: an unexpected EOF.
#[derive(Debug)]
pub enum ReadError {
    /// Unexpected end of file reached while reading bytes.
    UnexpectedEOF,
    /// I/O error occurred while reading bytes.
    Io(io::Error),
}

impl From<io::Error> for ReadError {
    fn from(err: io::Error) -> ReadError {
        ReadError::Io(err)
    }
}

impl From<byteorder::Error> for ReadError {
    fn from(err: byteorder::Error) -> ReadError {
        match err {
            byteorder::Error::UnexpectedEOF => ReadError::UnexpectedEOF,
            byteorder::Error::Io(err) => ReadError::Io(err),
        }
    }
}

/// Represents an error that can occur when attempting to read a MessagePack marker from the reader.
///
/// This is a thin wrapper over the standard `io::Error` type. Namely, it adds one additional error
/// case: an unexpected EOF.
#[derive(Debug)]
pub enum MarkerReadError {
    /// Unexpected end of file reached while reading the marker.
    UnexpectedEOF,
    /// I/O error occurred while reading the marker.
    Io(io::Error),
}

impl From<byteorder::Error> for MarkerReadError {
    fn from(err: byteorder::Error) -> MarkerReadError {
        match err {
            byteorder::Error::UnexpectedEOF => MarkerReadError::UnexpectedEOF,
            byteorder::Error::Io(err) => MarkerReadError::Io(err),
        }
    }
}

impl From<MarkerReadError> for ReadError {
    fn from(err: MarkerReadError) -> ReadError {
        match err {
            MarkerReadError::UnexpectedEOF => ReadError::UnexpectedEOF,
            MarkerReadError::Io(err) => ReadError::Io(err),
        }
    }
}

/// Represents an error that can occur when attempting to read a MessagePack'ed single-byte value
/// from the reader.
#[derive(Debug)]
pub enum FixedValueReadError {
    /// Unexpected end of file reached while reading the value.
    UnexpectedEOF,
    /// I/O error occurred while reading the value.
    Io(io::Error),
    /// The type decoded isn't match with the expected one.
    TypeMismatch(Marker),
}

impl From<MarkerReadError> for FixedValueReadError {
    fn from(err: MarkerReadError) -> FixedValueReadError {
        match err {
            MarkerReadError::UnexpectedEOF => FixedValueReadError::UnexpectedEOF,
            MarkerReadError::Io(err) => FixedValueReadError::Io(err),
        }
    }
}

/// Represents an error that can occur when attempting to read a MessagePack'ed complex value from
/// the reader.
#[derive(Debug)]
pub enum ValueReadError {
    /// Failed to read the marker.
    InvalidMarkerRead(ReadError),
    /// Failed to read the data.
    InvalidDataRead(ReadError),
    /// The type decoded isn't match with the expected one.
    TypeMismatch(Marker),
}

impl From<MarkerReadError> for ValueReadError {
    fn from(err: MarkerReadError) -> ValueReadError {
        ValueReadError::InvalidMarkerRead(From::from(err))
    }
}

#[derive(Debug)]
pub enum DecodeStringError<'a> {
    InvalidMarkerRead(ReadError),
    InvalidDataRead(ReadError),
    TypeMismatch(Marker),
    /// The given buffer is not large enough to accumulate the specified amount of bytes.
    BufferSizeTooSmall(u32),
    InvalidDataCopy(&'a [u8], ReadError),
    InvalidUtf8(&'a [u8], Utf8Error),
}

impl<'a> From<ValueReadError> for DecodeStringError<'a> {
    fn from(err: ValueReadError) -> DecodeStringError<'a> {
        match err {
            ValueReadError::InvalidMarkerRead(err) => DecodeStringError::InvalidMarkerRead(err),
            ValueReadError::InvalidDataRead(err) => DecodeStringError::InvalidDataRead(err),
            ValueReadError::TypeMismatch(marker) => DecodeStringError::TypeMismatch(marker),
        }
    }
}

/// Attempts to read a single byte from the given reader and decodes it as a MessagePack marker.
fn read_marker<R>(rd: &mut R) -> Result<Marker, MarkerReadError>
    where R: Read
{
    match rd.read_u8() {
        Ok(val)  => Ok(Marker::from_u8(val)),
        Err(err) => Err(From::from(err)),
    }
}

/// Attempts to read a single byte from the given reader and to decode it as a nil value.
///
/// According to the MessagePack specification, a nil value is represented as a single `0xc0` byte.
///
/// # Errors
///
/// This function will return `FixedValueReadError` on any I/O error while reading the nil marker.
///
/// It also returns `FixedValueReadError::TypeMismatch` if the actual type is not equal with the
/// expected one, indicating you with the actual type.
pub fn read_nil<R>(rd: &mut R) -> Result<(), FixedValueReadError>
    where R: Read
{
    match try!(read_marker(rd)) {
        Marker::Null => Ok(()),
        marker       => Err(FixedValueReadError::TypeMismatch(marker))
    }
}

/// Attempts to read a single byte from the given reader and to decode it as a boolean value.
///
/// According to the MessagePack specification, an encoded boolean value is represented as a single
/// byte.
///
/// # Errors
///
/// This function will return `FixedValueReadError` on any I/O error while reading the bool marker.
///
/// It also returns `FixedValueReadError::TypeMismatch` if the actual type is not equal with the
/// expected one, indicating you with the actual type.
pub fn read_bool<R>(rd: &mut R) -> Result<bool, FixedValueReadError>
    where R: Read
{
    match try!(read_marker(rd)) {
        Marker::True  => Ok(true),
        Marker::False => Ok(false),
        marker        => Err(FixedValueReadError::TypeMismatch(marker))
    }
}

/// Attempts to read a single byte from the given reader and to decode it as a positive fixnum
/// value.
///
/// According to the MessagePack specification, a positive fixed integer value is represented using
/// a single byte in `[0x00; 0x7f]` range inclusively, prepended with a special marker mask.
///
/// # Errors
///
/// This function will return `FixedValueReadError` on any I/O error while reading the marker.
///
/// It also returns `FixedValueReadError::TypeMismatch` if the actual type is not equal with the
/// expected one, indicating you with the actual type.
pub fn read_pfix<R>(rd: &mut R) -> Result<u8, FixedValueReadError>
    where R: Read
{
    match try!(read_marker(rd)) {
        Marker::PositiveFixnum(val) => Ok(val),
        marker => Err(FixedValueReadError::TypeMismatch(marker)),
    }
}

/// Attempts to read a single byte from the given reader and to decode it as a negative fixnum
/// value.
///
/// According to the MessagePack specification, a negative fixed integer value is represented using
/// a single byte in `[0xe0; 0xff]` range inclusively, prepended with a special marker mask.
///
/// # Errors
///
/// This function will return `FixedValueReadError` on any I/O error while reading the marker.
///
/// It also returns `FixedValueReadError::TypeMismatch` if the actual type is not equal with the
/// expected one, indicating you with the actual type.
pub fn read_nfix<R>(rd: &mut R) -> Result<i8, FixedValueReadError>
    where R: Read
{
    match try!(read_marker(rd)) {
        Marker::NegativeFixnum(val) => Ok(val),
        marker => Err(FixedValueReadError::TypeMismatch(marker)),
    }
}

macro_rules! make_read_data_fn {
    (deduce, $reader:ident, $decoder:ident, 0)
        => ($reader.$decoder(););
    (deduce, $reader:ident, $decoder:ident, 1)
        => ($reader.$decoder::<byteorder::BigEndian>(););
    (gen, $t:ty, $d:tt, $name:ident, $decoder:ident) => {
        fn $name<R>(rd: &mut R) -> Result<$t, ValueReadError>
            where R: Read
        {
            match make_read_data_fn!(deduce, rd, $decoder, $d) {
                Ok(data) => Ok(data),
                Err(err) => Err(ValueReadError::InvalidDataRead(From::from(err))),
            }
        }
    };
    (u8,    $name:ident, $decoder:ident) => (make_read_data_fn!(gen, u8, 0, $name, $decoder););
    (i8,    $name:ident, $decoder:ident) => (make_read_data_fn!(gen, i8, 0, $name, $decoder););
    ($t:ty, $name:ident, $decoder:ident) => (make_read_data_fn!(gen, $t, 1, $name, $decoder););
}

make_read_data_fn!(u8,  read_data_u8,  read_u8);
make_read_data_fn!(u16, read_data_u16, read_u16);
make_read_data_fn!(u32, read_data_u32, read_u32);
make_read_data_fn!(u64, read_data_u64, read_u64);
make_read_data_fn!(i8,  read_data_i8,  read_i8);
make_read_data_fn!(i16, read_data_i16, read_i16);
make_read_data_fn!(i32, read_data_i32, read_i32);
make_read_data_fn!(i64, read_data_i64, read_i64);
make_read_data_fn!(f32, read_data_f32, read_f32);
make_read_data_fn!(f64, read_data_f64, read_f64);

/// Attempts to read exactly 2 bytes from the given reader and to decode them as `u8` value.
///
/// The first byte should be the marker and the second one should represent the data itself.
///
/// # Errors
///
/// This function will return `ValueReadError` on any I/O error while reading either the marker or
/// the data.
///
/// It also returns `ValueReadError::TypeMismatch` if the actual type is not equal with the
/// expected one, indicating you with the actual type.
pub fn read_u8<R>(rd: &mut R) -> Result<u8, ValueReadError>
    where R: Read
{
    match try!(read_marker(rd)) {
        Marker::U8 => Ok(try!(read_data_u8(rd))),
        marker     => Err(ValueReadError::TypeMismatch(marker)),
    }
}

/// Attempts to read exactly 3 bytes from the given reader and to decode them as `u16` value.
///
/// The first byte should be the marker and the others should represent the data itself.
///
/// # Errors
///
/// This function will return `ValueReadError` on any I/O error while reading either the marker or
/// the data.
///
/// It also returns `ValueReadError::TypeMismatch` if the actual type is not equal with the
/// expected one, indicating you with the actual type.
pub fn read_u16<R>(rd: &mut R) -> Result<u16, ValueReadError>
    where R: Read
{
    match try!(read_marker(rd)) {
        Marker::U16 => Ok(try!(read_data_u16(rd))),
        marker      => Err(ValueReadError::TypeMismatch(marker)),
    }
}

/// Attempts to read exactly 5 bytes from the given reader and to decode them as `u32` value.
///
/// The first byte should be the marker and the others should represent the data itself.
///
/// # Errors
///
/// This function will return `ValueReadError` on any I/O error while reading either the marker or
/// the data.
///
/// It also returns `ValueReadError::TypeMismatch` if the actual type is not equal with the
/// expected one, indicating you with the actual type.
pub fn read_u32<R>(rd: &mut R) -> Result<u32, ValueReadError>
    where R: Read
{
    match try!(read_marker(rd)) {
        Marker::U32 => Ok(try!(read_data_u32(rd))),
        marker      => Err(ValueReadError::TypeMismatch(marker)),
    }
}

/// Attempts to read exactly 9 bytes from the given reader and to decode them as `u64` value.
///
/// The first byte should be the marker and the others should represent the data itself.
///
/// # Errors
///
/// This function will return `ValueReadError` on any I/O error while reading either the marker or
/// the data.
///
/// It also returns `ValueReadError::TypeMismatch` if the actual type is not equal with the
/// expected one, indicating you with the actual type.
pub fn read_u64<R>(rd: &mut R) -> Result<u64, ValueReadError>
    where R: Read
{
    match try!(read_marker(rd)) {
        Marker::U64 => Ok(try!(read_data_u64(rd))),
        marker      => Err(ValueReadError::TypeMismatch(marker)),
    }
}

/// Attempts to read exactly 2 bytes from the given reader and to decode them as `i8` value.
///
/// The first byte should be the marker and the second one should represent the data itself.
///
/// # Errors
///
/// This function will return `ValueReadError` on any I/O error while reading either the marker or
/// the data.
///
/// It also returns `ValueReadError::TypeMismatch` if the actual type is not equal with the
/// expected one, indicating you with the actual type.
pub fn read_i8<R>(rd: &mut R) -> Result<i8, ValueReadError>
    where R: Read
{
    match try!(read_marker(rd)) {
        Marker::I8 => Ok(try!(read_data_i8(rd))),
        marker     => Err(ValueReadError::TypeMismatch(marker)),
    }
}

/// Attempts to read exactly 3 bytes from the given reader and to decode them as `i16` value.
///
/// The first byte should be the marker and the others should represent the data itself.
///
/// # Errors
///
/// This function will return `ValueReadError` on any I/O error while reading either the marker or
/// the data.
///
/// It also returns `ValueReadError::TypeMismatch` if the actual type is not equal with the
/// expected one, indicating you with the actual type.
pub fn read_i16<R>(rd: &mut R) -> Result<i16, ValueReadError>
    where R: Read
{
    match try!(read_marker(rd)) {
        Marker::I16 => Ok(try!(read_data_i16(rd))),
        marker      => Err(ValueReadError::TypeMismatch(marker)),
    }
}

/// Attempts to read exactly 5 bytes from the given reader and to decode them as `i32` value.
///
/// The first byte should be the marker and the others should represent the data itself.
///
/// # Errors
///
/// This function will return `ValueReadError` on any I/O error while reading either the marker or
/// the data.
///
/// It also returns `ValueReadError::TypeMismatch` if the actual type is not equal with the
/// expected one, indicating you with the actual type.
pub fn read_i32<R>(rd: &mut R) -> Result<i32, ValueReadError>
    where R: Read
{
    match try!(read_marker(rd)) {
        Marker::I32 => Ok(try!(read_data_i32(rd))),
        marker      => Err(ValueReadError::TypeMismatch(marker)),
    }
}

/// Attempts to read exactly 9 bytes from the given reader and to decode them as `i64` value.
///
/// The first byte should be the marker and the others should represent the data itself.
///
/// # Errors
///
/// This function will return `ValueReadError` on any I/O error while reading either the marker or
/// the data.
///
/// It also returns `ValueReadError::TypeMismatch` if the actual type is not equal with the
/// expected one, indicating you with the actual type.
pub fn read_i64<R>(rd: &mut R) -> Result<i64, ValueReadError>
    where R: Read
{
    match try!(read_marker(rd)) {
        Marker::I64 => Ok(try!(read_data_i64(rd))),
        marker      => Err(ValueReadError::TypeMismatch(marker)),
    }
}

/// Attempts to read up to 2 bytes from the given reader and to decode them as `u8` value.
///
/// Unlike the `read_u8`, this function weakens type restrictions, allowing you to safely decode
/// packed values even if you aren't sure about the actual type.
///
/// Note, that trying to decode signed integers will result in `TypeMismatch` error even if the
/// value fits in `u8`.
///
/// # Errors
///
/// This function will return `ValueReadError` on any I/O error while reading either the marker or
/// the data.
///
/// It also returns `ValueReadError::TypeMismatch` if the actual type is not equal with the
/// expected one, indicating you with the actual type.
pub fn read_u8_loosely<R>(rd: &mut R) -> Result<u8, ValueReadError>
    where R: Read
{
    match try!(read_marker(rd)) {
        Marker::PositiveFixnum(val) => Ok(val),
        Marker::U8 => Ok(try!(read_data_u8(rd))),
        marker     => Err(ValueReadError::TypeMismatch(marker)),
    }
}

/// Attempts to read up to 3 bytes from the given reader and to decode them as `u16` value.
///
/// Unlike the `read_u16`, this function weakens type restrictions, allowing you to safely decode
/// packed values even if you aren't sure about the actual type.
///
/// Note, that trying to decode signed integers will result in `TypeMismatch` error even if the
/// value fits in `u16`.
///
/// # Errors
///
/// This function will return `ValueReadError` on any I/O error while reading either the marker or
/// the data.
///
/// It also returns `ValueReadError::TypeMismatch` if the actual type is not equal with the
/// expected one, indicating you with the actual type.
pub fn read_u16_loosely<R>(rd: &mut R) -> Result<u16, ValueReadError>
    where R: Read
{
    match try!(read_marker(rd)) {
        Marker::PositiveFixnum(val) => Ok(val as u16),
        Marker::U8  => Ok(try!(read_data_u8(rd)) as u16),
        Marker::U16 => Ok(try!(read_data_u16(rd))),
        marker      => Err(ValueReadError::TypeMismatch(marker)),
    }
}

/// Attempts to read up to 5 bytes from the given reader and to decode them as `u32` value.
///
/// Unlike the `read_u32`, this function weakens type restrictions, allowing you to safely decode
/// packed values even if you aren't sure about the actual type.
///
/// Note, that trying to decode signed integers will result in `TypeMismatch` error even if the
/// value fits in `u32`.
///
/// # Errors
///
/// This function will return `ValueReadError` on any I/O error while reading either the marker or
/// the data.
///
/// It also returns `ValueReadError::TypeMismatch` if the actual type is not equal with the
/// expected one, indicating you with the actual type.
pub fn read_u32_loosely<R>(rd: &mut R) -> Result<u32, ValueReadError>
    where R: Read
{
    match try!(read_marker(rd)) {
        Marker::PositiveFixnum(val) => Ok(val as u32),
        Marker::U8  => Ok(try!(read_data_u8(rd))  as u32),
        Marker::U16 => Ok(try!(read_data_u16(rd)) as u32),
        Marker::U32 => Ok(try!(read_data_u32(rd))),
        marker      => Err(ValueReadError::TypeMismatch(marker)),
    }
}

/// Attempts to read up to 9 bytes from the given reader and to decode them as `u64` value.
///
/// This function will try to read up to 9 bytes from the reader (1 for marker and up to 8 for data)
/// and interpret them as a big-endian u64.
///
/// Unlike the `read_u64`, this function weakens type restrictions, allowing you to safely decode
/// packed values even if you aren't sure about the actual type.
///
/// Note, that trying to decode signed integers will result in `TypeMismatch` error even if the
/// value fits in `u64`.
///
/// # Errors
///
/// This function will return `ValueReadError` on any I/O error while reading either the marker or
/// the data.
///
/// It also returns `ValueReadError::TypeMismatch` if the actual type is not equal with the
/// expected one, indicating you with the actual type.
pub fn read_u64_loosely<R>(rd: &mut R) -> Result<u64, ValueReadError>
    where R: Read
{
    match try!(read_marker(rd)) {
        Marker::PositiveFixnum(val) => Ok(val as u64),
        Marker::U8  => Ok(try!(read_data_u8(rd))  as u64),
        Marker::U16 => Ok(try!(read_data_u16(rd)) as u64),
        Marker::U32 => Ok(try!(read_data_u32(rd)) as u64),
        Marker::U64 => Ok(try!(read_data_u64(rd))),
        marker      => Err(ValueReadError::TypeMismatch(marker)),
    }
}

/// Attempts to read up to 2 bytes from the given reader and to decode them as `i8` value.
///
/// Unlike the `read_i8`, this function weakens type restrictions, allowing you to safely decode
/// packed values even if you aren't sure about the actual type.
///
/// Note, that trying to decode unsigned integers will result in `TypeMismatch` error even if the
/// value fits in `i8`.
///
/// # Errors
///
/// This function will return `ValueReadError` on any I/O error while reading either the marker or
/// the data.
///
/// It also returns `ValueReadError::TypeMismatch` if the actual type is not equal with the
/// expected one, indicating you with the actual type.
pub fn read_i8_loosely<R>(rd: &mut R) -> Result<i8, ValueReadError>
    where R: Read
{
    match try!(read_marker(rd)) {
        Marker::NegativeFixnum(val) => Ok(val),
        Marker::I8  => Ok(try!(read_data_i8(rd))),
        marker      => Err(ValueReadError::TypeMismatch(marker)),
    }
}

/// Attempts to read up to 3 bytes from the given reader and to decode them as `i16` value.
///
/// Unlike the `read_i16`, this function weakens type restrictions, allowing you to safely decode
/// packed values even if you aren't sure about the actual type.
///
/// Note, that trying to decode unsigned integers will result in `TypeMismatch` error even if the
/// value fits in `i16`.
///
/// # Errors
///
/// This function will return `ValueReadError` on any I/O error while reading either the marker or
/// the data.
///
/// It also returns `ValueReadError::TypeMismatch` if the actual type is not equal with the
/// expected one, indicating you with the actual type.
pub fn read_i16_loosely<R>(rd: &mut R) -> Result<i16, ValueReadError>
    where R: Read
{
    match try!(read_marker(rd)) {
        Marker::NegativeFixnum(val) => Ok(val as i16),
        Marker::I8  => Ok(try!(read_data_i8(rd)) as i16),
        Marker::I16 => Ok(try!(read_data_i16(rd))),
        marker      => Err(ValueReadError::TypeMismatch(marker)),
    }
}

/// Attempts to read up to 5 bytes from the given reader and to decode them as `i32` value.
///
/// Unlike the `read_i32`, this function weakens type restrictions, allowing you to safely decode
/// packed values even if you aren't sure about the actual type.
///
/// Note, that trying to decode unsigned integers will result in `TypeMismatch` error even if the
/// value fits in `i32`.
///
/// # Errors
///
/// This function will return `ValueReadError` on any I/O error while reading either the marker or
/// the data.
///
/// It also returns `ValueReadError::TypeMismatch` if the actual type is not equal with the
/// expected one, indicating you with the actual type.
pub fn read_i32_loosely<R>(rd: &mut R) -> Result<i32, ValueReadError>
    where R: Read
{
    match try!(read_marker(rd)) {
        Marker::NegativeFixnum(val) => Ok(val as i32),
        Marker::I8  => Ok(try!(read_data_i8(rd))  as i32),
        Marker::I16 => Ok(try!(read_data_i16(rd)) as i32),
        Marker::I32 => Ok(try!(read_data_i32(rd))),
        marker      => Err(ValueReadError::TypeMismatch(marker)),
    }
}

/// Attempts to read up to 9 bytes from the given reader and to decode them as `i64` value.
///
/// This function will try to read up to 9 bytes from the reader (1 for marker and up to 8 for data)
/// and interpret them as a big-endian i64.
///
/// Unlike the `read_i64`, this function weakens type restrictions, allowing you to safely decode
/// packed values even if you aren't sure about the actual type.
///
/// Note, that trying to decode signed integers will result in `TypeMismatch` error even if the
/// value fits in `i64`.
///
/// # Errors
///
/// This function will return `ValueReadError` on any I/O error while reading either the marker or
/// the data.
///
/// It also returns `ValueReadError::TypeMismatch` if the actual type is not equal with the
/// expected one, indicating you with the actual type.
pub fn read_i64_loosely<R>(rd: &mut R) -> Result<i64, ValueReadError>
    where R: Read
{
    match try!(read_marker(rd)) {
        Marker::NegativeFixnum(val) => Ok(val as i64),
        Marker::I8  => Ok(try!(read_data_i8(rd))  as i64),
        Marker::I16 => Ok(try!(read_data_i16(rd)) as i64),
        Marker::I32 => Ok(try!(read_data_i32(rd)) as i64),
        Marker::I64 => Ok(try!(read_data_i64(rd))),
        marker      => Err(ValueReadError::TypeMismatch(marker)),
    }
}

/// Attempts to read exactly 5 bytes from the given reader and to decode them as `f32` value.
///
/// The first byte should be the marker and the others should represent the data itself.
///
/// # Errors
///
/// This function will return `ValueReadError` on any I/O error while reading either the marker or
/// the data.
///
/// It also returns `ValueReadError::TypeMismatch` if the actual type is not equal with the
/// expected one, indicating you with the actual type.
pub fn read_f32<R>(rd: &mut R) -> Result<f32, ValueReadError>
    where R: Read
{
    match try!(read_marker(rd)) {
        Marker::F32 => Ok(try!(read_data_f32(rd))),
        marker      => Err(ValueReadError::TypeMismatch(marker))
    }
}

/// Attempts to read exactly 9 bytes from the given reader and to decode them as `f64` value.
///
/// The first byte should be the marker and the others should represent the data itself.
///
/// # Errors
///
/// This function will return `ValueReadError` on any I/O error while reading either the marker or
/// the data.
///
/// It also returns `ValueReadError::TypeMismatch` if the actual type is not equal with the
/// expected one, indicating you with the actual type.
pub fn read_f64<R>(rd: &mut R) -> Result<f64, ValueReadError>
    where R: Read
{
    match try!(read_marker(rd)) {
        Marker::F64 => Ok(try!(read_data_f64(rd))),
        marker      => Err(ValueReadError::TypeMismatch(marker))
    }
}

/// Attempts to read up to 9 bytes from the given reader and to decode them as a string `u32` size
/// value.
///
/// According to the MessagePack specification, the string format family stores an byte array in 1,
/// 2, 3, or 5 bytes of extra bytes in addition to the size of the byte array.
///
/// This function will return `ValueReadError` on any I/O error while reading either the marker or
/// the data.
///
/// It also returns `ValueReadError::TypeMismatch` if the actual type is not equal with the
/// expected one, indicating you with the actual type.
pub fn read_str_len<R>(rd: &mut R) -> Result<u32, ValueReadError>
    where R: Read
{
    match try!(read_marker(rd)) {
        Marker::FixedString(size) => Ok(size as u32),
        Marker::Str8  => Ok(try!(read_data_u8(rd))  as u32),
        Marker::Str16 => Ok(try!(read_data_u16(rd)) as u32),
        Marker::Str32 => Ok(try!(read_data_u32(rd))),
        marker        => Err(ValueReadError::TypeMismatch(marker))
    }
}

/// Attempts to read a string data from the given reader and copy it to the buffer provided.
///
/// On success returns a borrowed string type, allowing to view the copyed bytes as properly utf-8
/// string.
/// According to the spec, the string's data must to be encoded using utf-8.
///
/// # Errors
///
/// Returns `Err` in the following cases:
///
///  - if any IO error (including unexpected EOF) occurs, while reading an `rd`.
///  - if the `out` buffer size is not large enough to keep all the data copyed.
///  - if the data is not utf-8, with a description as to why the provided data is not utf-8 and
///    with a size of bytes actually copyed to be able to get them from `out`.
///
/// # Examples
/// ```
/// use msgpack::decode::read_str;
///
/// let buf = [0xaa, 0x6c, 0x65, 0x20, 0x6d, 0x65, 0x73, 0x73, 0x61, 0x67, 0x65];
/// let mut out = [0u8; 16];
///
/// assert_eq!("le message", read_str(&mut &buf[..], &mut &mut out[..]).unwrap());
/// ```
///
/// # Unstable
///
/// This function is **unstable**, because it needs review.
pub fn read_str<'r, R>(rd: &mut R, mut buf: &'r mut [u8]) -> Result<&'r str, DecodeStringError<'r>>
    where R: Read
{
    let len = try!(read_str_len(rd));
    let ulen = len as usize;

    if buf.len() < ulen {
        return Err(DecodeStringError::BufferSizeTooSmall(len))
    }

    read_str_data(rd, len, &mut buf[0..ulen])
}

fn read_str_data<'r, R>(rd: &mut R, len: u32, buf: &'r mut[u8]) -> Result<&'r str, DecodeStringError<'r>>
    where R: Read
{
    debug_assert_eq!(len as usize, buf.len());

    // We need cursor here, because in the common case we cannot guarantee, that copying will be
    // performed in a single step.
    let mut cur = Cursor::new(buf);

    // Trying to copy exact `len` bytes.
    match io::copy(&mut rd.take(len as u64), &mut cur) {
        Ok(size) if size == len as u64 => {
            // Release buffer owning from cursor.
            let buf = cur.into_inner();

            match from_utf8(buf) {
                Ok(decoded) => Ok(decoded),
                Err(err)    => Err(DecodeStringError::InvalidUtf8(buf, err)),
            }
        }
        Ok(size) => {
            let buf = cur.into_inner();
            Err(DecodeStringError::InvalidDataCopy(&buf[..size as usize], ReadError::UnexpectedEOF))
        }
        Err(err) => Err(DecodeStringError::InvalidDataRead(From::from(err))),
    }
}

/// Attempts to read and decode a string value from the reader, returning a borrowed slice from it.
///
// TODO: it is better to return &str; may panic on len mismatch; extend documentation.
pub fn read_str_ref(rd: &[u8]) -> Result<&[u8], DecodeStringError> {
    let mut cur = io::Cursor::new(rd);
    let len = try!(read_str_len(&mut cur));
    let start = cur.position() as usize;
    Ok(&rd[start .. start + len as usize])
}

/// Attempts to read up to 5 bytes from the given reader and to decode them as a big-endian u32
/// array size.
///
/// Array format family stores a sequence of elements in 1, 3, or 5 bytes of extra bytes in addition
/// to the elements.
// TODO: Docs.
pub fn read_array_size<R>(rd: &mut R) -> Result<u32, ValueReadError>
    where R: Read
{
    match try!(read_marker(rd)) {
        Marker::FixedArray(size) => Ok(size as u32),
        Marker::Array16 => Ok(try!(read_data_u16(rd)) as u32),
        Marker::Array32 => Ok(try!(read_data_u32(rd))),
        marker => Err(ValueReadError::TypeMismatch(marker))
    }
}

/// Attempts to read up to 5 bytes from the given reader and to decode them as a big-endian u32
/// map size.
///
/// Map format family stores a sequence of elements in 1, 3, or 5 bytes of extra bytes in addition
/// to the elements.
// TODO: Docs.
pub fn read_map_size<R>(rd: &mut R) -> Result<u32, ValueReadError>
    where R: Read
{
    match try!(read_marker(rd)) {
        Marker::FixedMap(size) => Ok(size as u32),
        Marker::Map16 => Ok(try!(read_data_u16(rd)) as u32),
        Marker::Map32 => Ok(try!(read_data_u32(rd))),
        marker => Err(ValueReadError::TypeMismatch(marker))
    }
}

// TODO: Docs.
pub fn read_bin_len<R>(rd: &mut R) -> Result<u32, ValueReadError>
    where R: Read
{
    match try!(read_marker(rd)) {
        Marker::Bin8  => Ok(try!(read_data_u8(rd)) as u32),
        Marker::Bin16 => Ok(try!(read_data_u16(rd)) as u32),
        Marker::Bin32 => Ok(try!(read_data_u32(rd))),
        marker        => Err(ValueReadError::TypeMismatch(marker))
    }
}

// TODO: Docs; not sure about naming.
pub fn read_bin_borrow(rd: &[u8]) -> Result<&[u8], ValueReadError> {
    let mut cur = io::Cursor::new(rd);
    let len = try!(read_bin_len(&mut cur)) as usize;

    let pos = cur.position() as usize;

    if rd.len() < pos + len {
        Err(ValueReadError::InvalidDataRead(ReadError::UnexpectedEOF))
    } else {
        Ok(&rd[pos .. pos + len])
    }
}

// TODO: Docs.
pub fn read_fixext1<R>(rd: &mut R) -> Result<(i8, u8), ValueReadError>
    where R: Read
{
    match try!(read_marker(rd)) {
        Marker::FixExt1 => {
            let id   = try!(read_data_i8(rd));
            let data = try!(read_data_u8(rd));
            Ok((id, data))
        }
        marker => Err(ValueReadError::TypeMismatch(marker))
    }
}

// TODO: Docs.
pub fn read_fixext2<R>(rd: &mut R) -> Result<(i8, u16), ValueReadError>
    where R: Read
{
    match try!(read_marker(rd)) {
        Marker::FixExt2 => {
            let id   = try!(read_data_i8(rd));
            let data = try!(read_data_u16(rd));
            Ok((id, data))
        }
        marker => Err(ValueReadError::TypeMismatch(marker))
    }
}

// TODO: Docs; contains unsafe code
pub fn read_fixext4<R>(rd: &mut R) -> Result<(i8, [u8; 4]), ValueReadError>
    where R: Read
{
    use std::mem;

    match try!(read_marker(rd)) {
        Marker::FixExt4 => {
            let id = try!(read_data_i8(rd));
            match rd.read_u32::<byteorder::LittleEndian>() {
                Ok(data) => {
                    let out : [u8; 4] = unsafe { mem::transmute(data) };
                    Ok((id, out))
                }
                Err(err) => Err(ValueReadError::InvalidDataRead(From::from(err))),
            }
        }
        _ => unimplemented!()
    }
}

// TODO: Docs, error cases, type mismatch, unsufficient bytes, extra bytes
pub fn read_fixext8<R>(rd: &mut R) -> Result<(i8, [u8; 8]), ValueReadError>
    where R: Read
{
    match try!(read_marker(rd)) {
        Marker::FixExt8 => {
            let id = try!(read_data_i8(rd));
            let mut out = [0u8; 8];

            match io::copy(&mut rd.take(8), &mut &mut out[..]) {
                Ok(8) => Ok((id, out)),
                _ => unimplemented!()
            }
        }
        _ => unimplemented!()
    }
}

// TODO: Docs, error cases, type mismatch, unsufficient bytes, extra bytes
pub fn read_fixext16<R>(rd: &mut R) -> Result<(i8, [u8; 16]), ValueReadError>
    where R: Read
{
    match try!(read_marker(rd)) {
        Marker::FixExt16 => {
            let id = try!(read_data_i8(rd));
            let mut out = [0u8; 16];

            match io::copy(&mut rd.take(16), &mut &mut out[..]) {
                Ok(16) => Ok((id, out)),
                _ => unimplemented!()
            }
        }
        _ => unimplemented!()
    }
}

#[derive(Debug, PartialEq)]
pub struct ExtMeta {
    pub typeid: i8,
    pub size: u32,
}

/// Unstable: docs, errors
pub fn read_ext_meta<R>(rd: &mut R) -> Result<ExtMeta, ValueReadError>
    where R: Read
{
    let size = match try!(read_marker(rd)) {
        Marker::FixExt1  => 1,
        Marker::FixExt2  => 2,
        Marker::FixExt4  => 4,
        Marker::FixExt8  => 8,
        Marker::FixExt16 => 16,
        Marker::Ext8     => try!(read_data_u8(rd))  as u32,
        Marker::Ext16    => try!(read_data_u16(rd)) as u32,
        Marker::Ext32    => try!(read_data_u32(rd)),
        _ => unimplemented!()
    };

    let typeid = try!(read_data_i8(rd));
    let meta = ExtMeta { typeid: typeid, size: size };

    Ok(meta)
}

////////////////////////////////////////////////////////////////////////////////////////////////////

///// Yes, it is slower, because of ADT, but more convenient.
/////
///// Unstable: move to high-level module; complete; test
//pub fn read_integer<R>(rd: &mut R) -> Result<Integer>
//    where R: Read
//{
//    match try!(read_marker(rd)) {
//        Marker::NegativeFixnum(val) => Ok(Integer::I64(val as i64)),
//        Marker::I8  => Ok(Integer::I64(try!(read_data_i8(rd))  as i64)),
//        Marker::I16 => Ok(Integer::I64(try!(read_data_i16(rd)) as i64)),
//        Marker::I32 => Ok(Integer::I64(try!(read_data_i32(rd)) as i64)),
//        Marker::I64 => Ok(Integer::I64(try!(read_data_i64(rd)))),
//        Marker::U64 => Ok(Integer::U64(try!(read_data_u64(rd)))),
//        marker      => Err(Error::TypeMismatch(marker)),
//    }
//}

/// TODO: Markdown.
/// Contains: owned value decoding, owned error; owned result.
//pub mod value {

//use std::convert;
//use std::io::Read;
//use std::result;
//use std::str::Utf8Error;

//use super::{read_marker, read_data_u8, read_data_i32, read_str_data};
//use super::super::{Marker, Value, Integer, ReadError, DecodeStringError};
//use super::super::super::core;

//#[derive(Debug, PartialEq)]
//pub enum Error {
//    Core(core::Error),
//    InvalidDataCopy(Vec<u8>, ReadError),
//    /// The decoded data is not valid UTF-8, provides the original data and the corresponding error.
//    InvalidUtf8(Vec<u8>, Utf8Error),
//}

//impl convert::From<core::Error> for Error {
//    fn from(err: core::Error) -> Error {
//        Error::Core(err)
//    }
//}

//impl<'a> convert::From<DecodeStringError<'a>> for Error {
//    fn from(err: DecodeStringError) -> Error {
//        match err {
//            DecodeStringError::Core(err) => Error::Core(err),
//            DecodeStringError::BufferSizeTooSmall(..) => unimplemented!(),
//            DecodeStringError::InvalidDataCopy(buf, err) => Error::InvalidDataCopy(buf.to_vec(), err),
//            DecodeStringError::InvalidUtf8(buf, err) => Error::InvalidUtf8(buf.to_vec(), err),
//        }
//    }
//}

//pub type Result<T> = result::Result<T, Error>;

///// Unstable: docs; examples; incomplete
//pub fn read_value<R>(rd: &mut R) -> Result<Value>
//    where R: Read
//{
//    match try!(read_marker(rd)) {
//        Marker::Null => Ok(Value::Null),
//        Marker::PositiveFixnum(v) => Ok(Value::Integer(Integer::U64(v as u64))),
//        Marker::I32  => Ok(Value::Integer(Integer::I64(try!(read_data_i32(rd)) as i64))),
//        // TODO: Other integers.
//        // TODO: Floats.
//        Marker::Str8 => {
//            let len = try!(read_data_u8(rd)) as u64;

//            let mut buf: Vec<u8> = (0..len).map(|_| 0u8).collect();

//            Ok(Value::String(try!(read_str_data(rd, len as u32, &mut buf[..])).to_string()))
//        }
//        // TODO: Other strings.
//        Marker::FixedArray(len) => {
//            let mut vec = Vec::with_capacity(len as usize);

//            for _ in 0..len {
//                vec.push(try!(read_value(rd)));
//            }

//            Ok(Value::Array(vec))
//        }
//        // TODO: Map/Bin/Ext.
//        _ => unimplemented!()
//    }
//}

//} // mod value


pub mod serialize {

use std::convert::From;
use std::io::Read;
use std::result;

use serialize;

use super::super::Marker;
use super::{
    ReadError,
    FixedValueReadError,
    ValueReadError,
    DecodeStringError,
    read_nil,
    read_bool,
    read_u8_loosely,
    read_u16_loosely,
    read_u32_loosely,
    read_u64_loosely,
    read_i8_loosely,
    read_i16_loosely,
    read_i32_loosely,
    read_i64_loosely,
    read_f32,
    read_f64,
    read_str_len,
    read_str_data,
    read_array_size,
    read_map_size,
};

/// Unstable: docs; incomplete
#[derive(Debug)]
pub enum Error {
    /// The actual value type isn't equal with the expected one.
    TypeMismatch(Marker),
    InvalidMarkerRead(ReadError),
    InvalidDataRead(ReadError),
    LengthMismatch(u32),
    /// Uncategorized error.
    Uncategorized(String),
}

impl From<FixedValueReadError> for Error {
    fn from(err: FixedValueReadError) -> Error {
        match err {
            FixedValueReadError::UnexpectedEOF => Error::InvalidMarkerRead(ReadError::UnexpectedEOF),
            FixedValueReadError::Io(err) => Error::InvalidMarkerRead(ReadError::Io(err)),
            FixedValueReadError::TypeMismatch(marker) => Error::TypeMismatch(marker),
        }
    }
}

impl From<ValueReadError> for Error {
    fn from(err: ValueReadError) -> Error {
        match err {
            ValueReadError::TypeMismatch(marker)   => Error::TypeMismatch(marker),
            ValueReadError::InvalidMarkerRead(err) => Error::InvalidMarkerRead(err),
            ValueReadError::InvalidDataRead(err)   => Error::InvalidDataRead(err),
        }
    }
}

/// Unstable: docs; incomplete
impl<'a> From<DecodeStringError<'a>> for Error {
    fn from(err: DecodeStringError) -> Error {
        match err {
            DecodeStringError::InvalidMarkerRead(err) => Error::InvalidMarkerRead(err),
            DecodeStringError::InvalidDataRead(err) => unimplemented!(),
            DecodeStringError::TypeMismatch(marker) => unimplemented!(),
            DecodeStringError::BufferSizeTooSmall(..) => unimplemented!(),
            DecodeStringError::InvalidDataCopy(..) => unimplemented!(),
            DecodeStringError::InvalidUtf8(..) => unimplemented!(),
        }
    }
}

pub type Result<T> = result::Result<T, Error>;

pub struct Decoder<R: Read> {
    rd: R,
}

impl<R: Read> Decoder<R> {
    pub fn new(rd: R) -> Decoder<R> {
        Decoder {
            rd: rd
        }
    }
}

#[allow(unused)]
/// Unstable: docs; examples; incomplete
impl<R: Read> serialize::Decoder for Decoder<R> {
    type Error = Error;

    fn read_nil(&mut self) -> Result<()> {
        Ok(try!(read_nil(&mut self.rd)))
    }

    fn read_bool(&mut self) -> Result<bool> {
        Ok(try!(read_bool(&mut self.rd)))
    }

    fn read_u8(&mut self) -> Result<u8> {
        Ok(try!(read_u8_loosely(&mut self.rd)))
    }

    fn read_u16(&mut self) -> Result<u16> {
        Ok(try!(read_u16_loosely(&mut self.rd)))
    }

    fn read_u32(&mut self) -> Result<u32> {
        Ok(try!(read_u32_loosely(&mut self.rd)))
    }

    fn read_u64(&mut self) -> Result<u64> {
        Ok(try!(read_u64_loosely(&mut self.rd)))
    }

    /// TODO: Doesn't look safe.
    fn read_usize(&mut self) -> Result<usize> {
        let v = try!(self.read_u64());
        Ok(v as usize)
    }

    fn read_i8(&mut self) -> Result<i8> {
        Ok(try!(read_i8_loosely(&mut self.rd)))
    }

    fn read_i16(&mut self) -> Result<i16> {
        Ok(try!(read_i16_loosely(&mut self.rd)))
    }

    fn read_i32(&mut self) -> Result<i32> {
        Ok(try!(read_i32_loosely(&mut self.rd)))
    }

    fn read_i64(&mut self) -> Result<i64> {
        Ok(try!(read_i64_loosely(&mut self.rd)))
    }

    /// TODO: Doesn't look safe.
    fn read_isize(&mut self) -> Result<isize> {
        Ok(try!(self.read_i64()) as isize)
    }

    fn read_f32(&mut self) -> Result<f32> {
        Ok(try!(read_f32(&mut self.rd)))
    }

    fn read_f64(&mut self) -> Result<f64> {
        Ok(try!(read_f64(&mut self.rd)))
    }

    fn read_char(&mut self) -> Result<char> { unimplemented!() }

    fn read_str(&mut self) -> Result<String> {
        let len = try!(read_str_len(&mut self.rd));

        let mut buf: Vec<u8> = (0..len).map(|_| 0u8).collect();

        Ok(try!(read_str_data(&mut self.rd, len, &mut buf[..])).to_string())
    }

    fn read_enum<T, F>(&mut self, name: &str, f: F) -> Result<T>
        where F: FnOnce(&mut Self) -> Result<T> { unimplemented!() }
    fn read_enum_variant<T, F>(&mut self, names: &[&str], f: F) -> Result<T>
        where F: FnMut(&mut Self, usize) -> Result<T> { unimplemented!() }
    fn read_enum_variant_arg<T, F>(&mut self, a_idx: usize, f: F) -> Result<T>
        where F: FnOnce(&mut Self) -> Result<T> { unimplemented!() }
    fn read_enum_struct_variant<T, F>(&mut self, names: &[&str], f: F) -> Result<T>
        where F: FnMut(&mut Self, usize) -> Result<T> { unimplemented!() }
    fn read_enum_struct_variant_field<T, F>(&mut self, f_name: &str, f_idx: usize, f: F) -> Result<T>
        where F: FnOnce(&mut Self) -> Result<T> { unimplemented!() }

    fn read_struct<T, F>(&mut self, name_: &str, len: usize, f: F) -> Result<T>
        where F: FnOnce(&mut Self) -> Result<T>
    {
        self.read_tuple(len, f)
    }

    fn read_struct_field<T, F>(&mut self, name_: &str, idx_: usize, f: F) -> Result<T>
        where F: FnOnce(&mut Self) -> Result<T>
    {
        f(self)
    }

    fn read_tuple<T, F>(&mut self, len: usize, f: F) -> Result<T>
        where F: FnOnce(&mut Self) -> Result<T>
    {
        let actual = try!(read_array_size(&mut self.rd));

        if len == actual as usize {
            f(self)
        } else {
            Err(Error::LengthMismatch(actual))
        }
    }

    // In case of MessagePack don't care about argument indexing.
    fn read_tuple_arg<T, F>(&mut self, idx_: usize, f: F) -> Result<T>
        where F: FnOnce(&mut Self) -> Result<T>
    {
        f(self)
    }

    fn read_tuple_struct<T, F>(&mut self, s_name: &str, len: usize, f: F) -> Result<T>
        where F: FnOnce(&mut Self) -> Result<T> { unimplemented!() }
    fn read_tuple_struct_arg<T, F>(&mut self, a_idx: usize, f: F) -> Result<T>
        where F: FnOnce(&mut Self) -> Result<T> { unimplemented!() }

    /// We treat Value::Null as None.
    fn read_option<T, F>(&mut self, mut f: F) -> Result<T>
        where F: FnMut(&mut Self, bool) -> Result<T>
    {
        // Primarily try to read optimisticly.
        match f(self, true) {
            Ok(val) => Ok(val),
            Err(Error::TypeMismatch(Marker::Null)) => f(self, false),
            Err(err) => Err(err)
        }
    }

    fn read_seq<T, F>(&mut self, f: F) -> Result<T>
        where F: FnOnce(&mut Self, usize) -> Result<T>
    {
        let len = try!(read_array_size(&mut self.rd)) as usize;

        f(self, len)
    }

    fn read_seq_elt<T, F>(&mut self, idx_: usize, f: F) -> Result<T>
        where F: FnOnce(&mut Self) -> Result<T>
    {
        f(self)
    }

    fn read_map<T, F>(&mut self, f: F) -> Result<T>
        where F: FnOnce(&mut Self, usize) -> Result<T>
    {
        let len = try!(read_map_size(&mut self.rd)) as usize;

        f(self, len)
    }

    fn read_map_elt_key<T, F>(&mut self, idx_: usize, f: F) -> Result<T>
        where F: FnOnce(&mut Self) -> Result<T>
    {
        f(self)
    }

    fn read_map_elt_val<T, F>(&mut self, idx_: usize, f: F) -> Result<T>
        where F: FnOnce(&mut Self) -> Result<T>
    {
        f(self)
    }

    fn error(&mut self, err: &str) -> Error {
        Error::Uncategorized(err.to_string())
    }
}

}
