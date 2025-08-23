use std::{
    fmt::{Display, Write},
    str::Chars,
};

#[derive(Debug, Clone)]
pub struct ToKebabCase<I> {
    first: bool,
    pending: Option<char>,
    input: I,
}

// TODO: rename to kebab case
#[derive(Debug, Clone)]
pub struct FromKebabCase {
    first: bool,
    sep: bool,
    buffer: String,
}

pub trait AsCase {
    type Iter;
    fn to_kebab_case(self) -> ToKebabCase<Self::Iter>;
}

impl Default for FromKebabCase {
    fn default() -> Self {
        Self::new()
    }
}
impl FromKebabCase {
    pub fn new() -> Self {
        Self {
            first: true,
            sep: false,
            buffer: String::new(),
        }
    }
    pub fn push(&mut self, c: char) {
        if c == '-' {
            self.sep = true;
            self.first = false;
            return;
        }
        if self.first || self.sep {
            self.buffer.extend(c.to_uppercase());
        } else {
            self.buffer.push(c);
        }
        self.first = false;
        self.sep = false;
    }
}
impl Display for FromKebabCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.buffer)
    }
}

impl<'a> AsCase for &'a str {
    type Iter = Chars<'a>;

    fn to_kebab_case(self) -> ToKebabCase<Self::Iter> {
        ToKebabCase::new(self.chars())
    }
}

impl<I> ToKebabCase<I>
where
    I: Iterator<Item = char>,
{
    pub fn new(input: impl IntoIterator<IntoIter = I>) -> Self {
        Self {
            first: true,
            pending: None,
            input: input.into_iter(),
        }
    }
}

impl<I> Iterator for ToKebabCase<I>
where
    I: Iterator<Item = char>,
{
    type Item = char;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(pending) = self.pending.take() {
            return Some(pending);
        }
        let c = self.input.next()?;
        let sep = c.is_uppercase() && !self.first;
        self.first = false;
        let c = c.to_ascii_lowercase();
        if sep {
            self.pending = Some(c);
            Some('-')
        } else {
            Some(c)
        }
    }
}

impl<I> Display for ToKebabCase<I>
where
    I: Iterator<Item = char> + Clone,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for c in self.clone() {
            f.write_char(c)?;
        }
        Ok(())
    }
}
