use super::error::{Error, Result};
use ascase::{AsCase, FromSnakeCase};
use codepoint::{next_code_point, try_next_code_point};
use serde::{
    Deserialize,
    de::{DeserializeSeed, EnumAccess, MapAccess, SeqAccess, VariantAccess, Visitor},
};
use std::{io::Read, str::Bytes};

pub trait Reader {
    fn next_char(&mut self) -> Result<Option<char>>;
}

// struct ReaderFromChars<T>(T);
struct ReaderFromBytes<T>(T);
struct ReaderFromIo<T>(T);

// impl<I> Reader for ReaderFromChars<I>
// where
//     I: Iterator<Item = char>,
// {
//     fn next_char(&mut self) -> Result<Option<char>> {
//         Ok(self.0.next())
//     }
// }

impl<I> Reader for ReaderFromBytes<I>
where
    I: Iterator<Item = u8>,
{
    fn next_char(&mut self) -> Result<Option<char>> {
        next_code_point(&mut self.0, Error::InvalidUtf8)
    }
}

impl<I> Reader for ReaderFromIo<I>
where
    I: Iterator<Item = std::io::Result<u8>>,
{
    fn next_char(&mut self) -> Result<Option<char>> {
        try_next_code_point(
            &mut (&mut self.0).map(|b| b.map_err(|_| Error::Io)),
            Error::InvalidUtf8,
        )
    }
}

pub struct Deserializer<R: Reader> {
    col: usize,
    row: usize,
    peeked: Result<Option<char>>,
    input: R,
}

impl<R> Deserializer<R>
where
    R: Reader,
{
    pub fn new(mut input: R) -> Self {
        let peeked = input.next_char();
        Self {
            peeked,
            input,
            col: 0,
            row: 0,
        }
    }
}

impl<'a> Deserializer<ReaderFromBytes<Bytes<'a>>> {
    pub fn from_str(input: &'a str) -> Self {
        Self::new(ReaderFromBytes(input.bytes()))
    }
}

pub fn from_reader<'a, R, T>(reader: R) -> Result<T>
where
    T: Deserialize<'a>,
    R: Read,
{
    let mut deserializer = Deserializer::new(ReaderFromIo(reader.bytes()));
    let t = T::deserialize(&mut deserializer)?;
    deserializer.skip_whitespace()?;
    if deserializer.peeked?.is_none() {
        Ok(t)
    } else {
        Err(Error::TrailingCharacters)
    }
}

pub fn from_str<'a, T>(s: &'a str) -> Result<T>
where
    T: Deserialize<'a>,
{
    let mut deserializer = Deserializer::from_str(s);
    let t = T::deserialize(&mut deserializer)?;
    deserializer.skip_whitespace()?;
    if deserializer.peeked?.is_none() {
        Ok(t)
    } else {
        Err(Error::TrailingCharacters)
    }
}
impl<R: Reader> Deserializer<R> {
    fn peek_char(&mut self) -> Option<char> {
        if let Ok(peeked) = self.peeked {
            peeked
        } else {
            None
        }
    }

    fn next_char(&mut self) -> Result<char> {
        let next = self.peeked.clone()?.take().ok_or(Error::Eof)?;
        if next == '\n' {
            self.row += 1;
            self.col = 0;
        } else {
            self.col += 1;
        }
        self.peeked = self.input.next_char();
        Ok(next)
    }

    fn next_if(&mut self, p: impl FnOnce(char) -> bool) -> Result<Option<char>> {
        match self.peeked {
            Err(ref err) => Err(err.clone()),
            Ok(None) => Ok(None),
            Ok(Some(peeked)) if p(peeked) => self.next_char().map(Some),
            Ok(Some(_)) => Ok(None),
        }
    }

    fn skip_whitespace(&mut self) -> Result<()> {
        while self.next_if(|c| c.is_whitespace())?.is_some() {}
        Ok(())
    }

    fn parse_ident(&mut self) -> Result<String> {
        self.skip_whitespace()?;
        let mut ident = FromSnakeCase::new();
        while let Some(c) = self.next_if(|c| c.is_alphanumeric() || c == '-')? {
            ident.push(c);
        }
        let ident = ident.to_string();
        if ident.is_empty() {
            return Err(Error::ExpectedIdentifier);
        }
        Ok(ident)
    }

    fn parse_hex(&mut self) -> Result<u64> {
        let mut acc: u64 = 0;
        while let Some(digit) = self.next_if(|c| matches!(c, '0'..='9' | 'a'..='f'))? {
            let digit = match digit {
                '0'..='9' => digit as u64 - '0' as u64 + 0,
                'a'..='f' => digit as u64 - 'a' as u64 + 10,
                _ => unreachable!(),
            };
            acc = acc.checked_mul(16).ok_or(Error::NumberOverflow)?;
            acc = acc.checked_add(digit).ok_or(Error::NumberOverflow)?;
        }
        Ok(acc)
    }

    fn parse_nat<T: TryFrom<u64>>(&mut self) -> Result<T> {
        self.parse_u64()?
            .try_into()
            .ok()
            .ok_or(Error::NumberOverflow)
    }
    fn parse_int<T: TryFrom<i64>>(&mut self) -> Result<T> {
        self.parse_i64()?
            .try_into()
            .ok()
            .ok_or(Error::NumberOverflow)
    }

    fn parse_u64(&mut self) -> Result<u64> {
        self.skip_whitespace()?;

        let first = self.peek_char().ok_or(Error::Eof)?;
        let '0'..='9' = self.peek_char().ok_or(Error::Eof)? else {
            return Err(Error::ExpectedChar {
                one_of: ('0'..='9').collect(),
                found: first,
                row: self.row,
                col: self.col,
            });
        };

        let parse_digit = |d| d as u64 - '0' as u64;
        let mut acc: u64 = 0;
        while let Some('0'..='9') = self.peek_char() {
            acc = acc.checked_mul(10).ok_or(Error::NumberOverflow)?;
            acc = acc
                .checked_add(parse_digit(self.next_char().unwrap()))
                .ok_or(Error::NumberOverflow)?;
        }
        Ok(acc)
    }

    fn parse_i64(&mut self) -> Result<i64> {
        self.skip_whitespace()?;
        let signum = match self.expects_either('+', '-')? {
            '+' => 1,
            '-' => -1,
            _ => unreachable!(),
        };
        let parse_digit = |d| d as i64 - '0' as i64;
        let mut acc: i64 = 0;

        let first = self.peek_char().ok_or(Error::Eof)?;
        let '0'..='9' = self.peek_char().ok_or(Error::Eof)? else {
            return Err(Error::ExpectedChar {
                one_of: ('0'..='9').collect(),
                found: first,
                col: self.col,
                row: self.row,
            });
        };

        while let Some('0'..='9') = self.peek_char() {
            acc = acc.checked_mul(10).ok_or(Error::NumberOverflow)?;
            acc = acc
                .checked_add(parse_digit(self.next_char().unwrap()))
                .ok_or(Error::NumberOverflow)?;
        }
        Ok(signum * acc)
    }

    fn expects_either(&mut self, lhs: char, rhs: char) -> Result<char> {
        self.skip_whitespace()?;
        let col = self.col;
        let row = self.row;
        match self.next_char()? {
            c if c == lhs => Ok(c),
            c if c == rhs => Ok(c),
            c => Err(Error::ExpectedChar {
                one_of: Vec::from([lhs, rhs]),
                found: c,
                col,
                row,
            }),
        }
    }

    fn expects(&mut self, iter: impl IntoIterator<Item = char>) -> Result<()> {
        self.skip_whitespace()?;
        self.expects_imm(iter)
    }
    fn expects_imm(&mut self, iter: impl IntoIterator<Item = char>) -> Result<()> {
        let col = self.col;
        let row = self.row;
        for c in iter {
            let found = self.next_char()?;
            if found != c {
                return Err(Error::ExpectedChar {
                    one_of: Vec::from([c]),
                    found,
                    col,
                    row,
                });
            }
        }
        Ok(())
    }
    fn parse_escape(&mut self) -> Result<char> {
        match self.next_char()? {
            '\'' => Ok('\''),
            '"' => Ok('\"'),
            'n' => Ok('\n'),
            '\\' => Ok('\\'),
            '{' => {
                let u = u32::try_from(self.parse_hex()?)
                    .ok()
                    .ok_or(Error::NumberOverflow)?;
                let c = char::try_from(u).ok().ok_or(Error::NumberOverflow)?;
                self.expects_imm(['}'])?;
                Ok(c)
            }
            c => Err(Error::InvalidEscape(c)),
        }
    }
}

impl<'a, 'de, R: Reader> serde::de::Deserializer<'de> for &'a mut Deserializer<R> {
    type Error = Error;

    fn deserialize_any<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        // match self.peek_char()? {
        //     'n' => self.deserialize_unit(visitor),
        //     't' | 'f' => self.deserialize_bool(visitor),
        //     '"' => self.deserialize_str(visitor),
        //     '0'..='9' => self.deserialize_u64(visitor),
        //     '-' => self.deserialize_i64(visitor),
        //     '[' => self.deserialize_seq(visitor),
        //     '{' => self.deserialize_map(visitor),
        //     _ => Err(Error::Syntax),
        // }
        unimplemented!()
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.expects_either('t', 'f')? {
            't' => {
                self.expects_imm("rue".chars())?;
                visitor.visit_bool(true)
            }
            'f' => {
                self.expects_imm("alse".chars())?;
                visitor.visit_bool(false)
            }
            _ => unreachable!(),
        }
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i8(self.parse_int()?)
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i16(self.parse_int()?)
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i32(self.parse_int()?)
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i64(self.parse_int()?)
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u8(self.parse_nat()?)
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u16(self.parse_nat()?)
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u32(self.parse_nat()?)
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u64(self.parse_nat()?)
    }

    fn deserialize_f32<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }

    fn deserialize_f64<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.expects(['\''])?;
        let value = visitor.visit_char::<Error>(match self.next_char()? {
            '\\' => self.parse_escape()?,
            c => c,
        })?;
        self.expects_imm(['\''])?;
        Ok(value)
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.expects(['\"'])?;
        let mut string = String::new();
        loop {
            match self.next_char()? {
                '"' => return visitor.visit_string(string),
                '\\' => string.push(self.parse_escape()?),
                c => string.push(c),
            }
        }
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_bytes<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }

    fn deserialize_byte_buf<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.expects("(".chars())?;
        let value = match self.expects_either('s', 'n')? {
            's' => {
                self.expects_imm("ome".chars())?;
                visitor.visit_some(&mut *self)?
            }
            'n' => {
                self.expects_imm("one".chars())?;
                visitor.visit_none::<Error>()?
            }
            _ => unreachable!(),
        };
        self.expects(")".chars())?;
        Ok(value)
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.expects("(".chars())?;
        self.expects("unit".chars())?;
        self.expects(")".chars())?;
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_unit(visitor)
    }

    fn deserialize_newtype_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.expects("[".chars())?;
        let value = visitor.visit_seq(&mut *self)?;
        self.expects("]".chars())?;
        Ok(value)
    }

    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.expects("(".chars())?;
        self.expects("map".chars())?;
        let value = visitor.visit_map(&mut *self)?;
        self.expects(")".chars())?;
        Ok(value)
    }

    fn deserialize_struct<V>(
        self,
        name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.expects("(".chars())?;
        self.expects(name.as_snake_case())?;
        let value = visitor.visit_seq(&mut *self)?;
        self.expects(")".chars())?;
        Ok(value)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.expects("(".chars())?;
        let value = visitor.visit_enum(&mut *self)?;
        self.expects(")".chars())?;
        Ok(value)
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_string(self.parse_ident()?)
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }
}

impl<'de, R: Reader> SeqAccess<'de> for Deserializer<R> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: DeserializeSeed<'de>,
    {
        self.skip_whitespace()?;
        if let Some(')' | ']') = self.peek_char() {
            return Ok(None);
        }
        seed.deserialize(&mut *self).map(Some)
    }
}

impl<'de, R: Reader> MapAccess<'de> for Deserializer<R> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>>
    where
        K: DeserializeSeed<'de>,
    {
        self.skip_whitespace()?;
        if self.peek_char() == Some(')') {
            return Ok(None);
        }
        self.expects("[".chars())?;
        seed.deserialize(&mut *self).map(Some)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
    where
        V: DeserializeSeed<'de>,
    {
        self.skip_whitespace()?;
        let value = seed.deserialize(&mut *self)?;
        self.expects("]".chars())?;
        Ok(value)
    }
}

impl<'de, 'a, R: Reader> EnumAccess<'de> for &'a mut Deserializer<R> {
    type Error = Error;
    type Variant = Self;
    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self)>
    where
        V: DeserializeSeed<'de>,
    {
        Ok((seed.deserialize(&mut *self)?, self))
    }
}

impl<'a, 'de, R: Reader> VariantAccess<'de> for &'a mut Deserializer<R> {
    type Error = Error;

    fn unit_variant(self) -> Result<()> {
        Ok(())
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value>
    where
        T: DeserializeSeed<'de>,
    {
        seed.deserialize(self)
    }

    fn tuple_variant<V>(self, _len: usize, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_seq(self)
    }

    fn struct_variant<V>(self, _fields: &'static [&'static str], visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_seq(self)
    }
}
