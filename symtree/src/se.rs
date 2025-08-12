use super::Error;
use ascase::AsCase;
use serde::ser::SerializeSeq;
use serde::{Serialize, ser};
use std::io::Write;

type Result<T> = std::result::Result<T, Error>;

pub struct Serializer<W>
where
    W: Write,
{
    sep: Sep,
    indent: Option<usize>,
    output: W,
}

#[derive(Clone, Copy)]
enum Sep {
    None,
    Line,
    LineOrSpace,
}

pub fn to_string<T>(value: &T) -> Result<String>
where
    T: Serialize,
{
    let mut serializer = Serializer {
        sep: Sep::None,
        indent: None,
        output: Vec::new(),
    };
    value.serialize(&mut serializer)?;
    Ok(serializer.output.try_into().unwrap())
}

pub fn to_writer<T: Serialize>(value: &T, writer: impl Write) -> Result<()> {
    let mut serializer = Serializer {
        sep: Sep::None,
        indent: None,
        output: writer,
    };
    value.serialize(&mut serializer)
}

pub fn to_writer_pretty<T: Serialize>(value: &T, writer: impl Write) -> Result<()> {
    let mut serializer = Serializer {
        sep: Sep::None,
        indent: Some(0),
        output: writer,
    };
    value.serialize(&mut serializer)?;
    writeln!(serializer.output).map_err(|_| Error::Io)
}

impl<W: Write> Serializer<W> {
    fn ensure_spacing(&mut self) -> Result<()> {
        match (self.sep, self.indent) {
            (Sep::LineOrSpace | Sep::Line, Some(level)) => {
                write!(self.output, "\n")?;
                for _ in 0..level {
                    write!(self.output, "  ")?;
                }
            }
            (Sep::Line | Sep::None, _) => {}
            (Sep::LineOrSpace, _) => {
                write!(self.output, " ")?;
            }
        }
        self.sep = Sep::None;
        Ok(())
    }
    fn indent(&mut self) {
        if let Some(level) = &mut self.indent {
            *level += 1;
        }
    }
    fn dedent(&mut self) {
        if let Some(level) = &mut self.indent {
            *level -= 1;
        }
    }
}

impl<'a, W> ser::Serializer for &'a mut Serializer<W>
where
    W: Write,
{
    type Ok = ();
    type Error = Error;
    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    fn serialize_bool(self, v: bool) -> Result<()> {
        write!(self.output, "{}", if v { "true" } else { "false" })?;
        Ok(())
    }

    fn serialize_i8(self, v: i8) -> Result<()> {
        write!(self.output, "{:+}", v)?;
        Ok(())
    }

    fn serialize_i16(self, v: i16) -> Result<()> {
        write!(self.output, "{:+}", v)?;
        Ok(())
    }

    fn serialize_i32(self, v: i32) -> Result<()> {
        write!(self.output, "{:+}", v)?;
        Ok(())
    }

    fn serialize_i64(self, v: i64) -> Result<()> {
        write!(self.output, "{:+}", v)?;
        Ok(())
    }

    fn serialize_u8(self, v: u8) -> Result<()> {
        write!(self.output, "{}", v)?;
        Ok(())
    }

    fn serialize_u16(self, v: u16) -> Result<()> {
        write!(self.output, "{}", v)?;
        Ok(())
    }

    fn serialize_u32(self, v: u32) -> Result<()> {
        write!(self.output, "{}", v)?;
        Ok(())
    }

    fn serialize_u64(self, v: u64) -> Result<()> {
        write!(self.output, "{}", v)?;
        Ok(())
    }

    fn serialize_f32(self, _: f32) -> Result<()> {
        // v.to_le_bytes()
        unimplemented!()
    }

    fn serialize_f64(self, _: f64) -> Result<()> {
        // v.to_le_bytes()
        unimplemented!()
    }

    fn serialize_char(self, v: char) -> Result<()> {
        // TODO: remove debug notation
        write!(self.output, "{:?}", v)?;
        Ok(())
    }

    fn serialize_str(self, v: &str) -> Result<()> {
        write!(self.output, "{:?}", v)?;
        Ok(())
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<()> {
        // TODO: use notation `{10af8342}`
        let mut seq = self.serialize_seq(Some(v.len()))?;
        for byte in v {
            seq.serialize_element(byte)?;
        }
        seq.end()?;
        Ok(())
    }

    fn serialize_none(self) -> Result<()> {
        write!(self.output, "(none)")?;
        Ok(())
    }

    fn serialize_some<T>(self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        write!(self.output, "(some")?;
        self.sep = Sep::LineOrSpace;
        self.indent();
        self.ensure_spacing()?;
        value.serialize(&mut *self)?;
        write!(self.output, ")")?;
        self.dedent();
        self.sep = Sep::LineOrSpace;
        Ok(())
    }

    fn serialize_unit(self) -> Result<()> {
        write!(self.output, "(unit)")?;
        Ok(())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        self.serialize_unit()
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<()> {
        write!(self.output, "({})", variant.as_snake_case())?;
        Ok(())
    }

    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        unimplemented!();
    }

    fn serialize_seq(self, _: Option<usize>) -> Result<Self::SerializeSeq> {
        write!(self.output, "[")?;
        self.indent();
        self.sep = Sep::Line;
        Ok(self)
    }

    fn serialize_tuple(self, _: usize) -> Result<Self::SerializeTuple> {
        write!(self.output, "[")?;
        self.indent();
        self.sep = Sep::Line;
        Ok(self)
    }

    // Tuple structs look just like sequences in JSON.
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        self.serialize_seq(Some(len))
    }

    // Tuple variants are represented in JSON as `{ NAME: [DATA...] }`. Again
    // this method is only responsible for the externally tagged representation.
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        unimplemented!();
    }
    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap> {
        write!(self.output, "(map")?;
        self.indent();
        self.sep = Sep::LineOrSpace;
        Ok(self)
    }
    fn serialize_struct(self, name: &'static str, _len: usize) -> Result<Self::SerializeStruct> {
        write!(self.output, "({}", name.as_snake_case())?;
        self.indent();
        self.sep = Sep::LineOrSpace;
        Ok(self)
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        write!(self.output, "({}", variant.as_snake_case())?;
        self.indent();
        self.sep = Sep::LineOrSpace;
        Ok(self)
    }
}

impl<'a, W> ser::SerializeSeq for &'a mut Serializer<W>
where
    W: Write,
{
    type Ok = ();
    type Error = Error;
    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.ensure_spacing()?;
        let indent = self.indent.take();
        value.serialize(&mut **self)?;
        self.indent = indent;
        self.sep = Sep::LineOrSpace;
        Ok(())
    }
    fn end(self) -> Result<()> {
        write!(self.output, "]")?;
        self.dedent();
        self.sep = Sep::LineOrSpace;
        Ok(())
    }
}

impl<'a, W> ser::SerializeTuple for &'a mut Serializer<W>
where
    W: Write,
{
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.ensure_spacing()?;
        value.serialize(&mut **self)?;
        self.sep = Sep::LineOrSpace;
        Ok(())
    }

    fn end(self) -> Result<()> {
        write!(self.output, "]")?;
        self.dedent();
        self.sep = Sep::LineOrSpace;
        Ok(())
    }
}

impl<'a, W> ser::SerializeTupleStruct for &'a mut Serializer<W>
where
    W: Write,
{
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.ensure_spacing()?;
        value.serialize(&mut **self)?;
        self.sep = Sep::LineOrSpace;
        Ok(())
    }

    fn end(self) -> Result<()> {
        write!(self.output, "]").unwrap();
        self.dedent();
        Ok(())
    }
}

impl<'a, W> ser::SerializeTupleVariant for &'a mut Serializer<W>
where
    W: Write,
{
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        unimplemented!()
    }
}

impl<'a, W> ser::SerializeMap for &'a mut Serializer<W>
where
    W: Write,
{
    type Ok = ();
    type Error = Error;

    fn serialize_key<T>(&mut self, key: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.ensure_spacing()?;
        write!(self.output, "[")?;
        self.sep = Sep::Line;
        let indent = self.indent.take();
        self.ensure_spacing()?;
        key.serialize(&mut **self)?;
        self.indent = indent;
        Ok(())
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.sep = Sep::LineOrSpace;
        let indent = self.indent.take();
        self.ensure_spacing()?;
        value.serialize(&mut **self)?;
        self.indent = indent;
        write!(self.output, "]")?;
        Ok(())
    }

    fn end(self) -> Result<()> {
        write!(self.output, ")")?;
        self.dedent();
        Ok(())
    }
}

impl<'a, W> ser::SerializeStruct for &'a mut Serializer<W>
where
    W: Write,
{
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, _key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.ensure_spacing()?;
        value.serialize(&mut **self)?;
        self.sep = Sep::LineOrSpace;
        Ok(())
    }

    fn end(self) -> Result<()> {
        write!(self.output, ")")?;
        self.dedent();
        Ok(())
    }
}

impl<'a, W> ser::SerializeStructVariant for &'a mut Serializer<W>
where
    W: Write,
{
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, _key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.ensure_spacing()?;
        value.serialize(&mut **self)?;
        self.sep = Sep::LineOrSpace;
        Ok(())
    }

    fn end(self) -> Result<()> {
        write!(self.output, ")")?;
        self.dedent();
        Ok(())
    }
}
