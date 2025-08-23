use super::{Error, Person, Span, validate};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use slab::Slab;
use std::{cmp::Ordering, ops::Range};
use time_util::{DaySpan, TimeZoneExt};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct State {
    pub time_zone: Tz,
    persons: Slab<Person>,
}

impl State {
    pub fn person(&self, person: u32) -> Result<&Person, Error> {
        self.persons
            .get(person as usize)
            .ok_or(Error::InvalidPerson(person))
    }
    pub fn new_person(&mut self, names: Vec<String>, admin: bool) -> u32 {
        self.persons.insert(Person {
            names,
            admin,
            spans: Vec::new(),
            entered: None,
        }) as u32
    }
    pub fn enters(&mut self, person: u32, enter: i64) -> Result<Option<i64>, Error> {
        validate::person(person, self)?;
        Ok(self.persons[person as usize].entered.replace(enter))
    }
    pub fn leaves(&mut self, person: u32, leave: i64) -> Result<Vec<Span>, Error> {
        validate::person(person, self)?;
        let Some(enter) = self.persons[person as usize].entered else {
            return Err(Error::NotEnteredYet);
        };
        validate::span(enter, leave, self)?;
        self.persons[person as usize].entered = None;
        self.add_span(person, enter, leave)
    }
    pub fn set_person_names(&mut self, person: u32, names: Vec<String>) -> Result<(), Error> {
        validate::person(person, self)?;
        self.persons[person as usize].names = names;
        Ok(())
    }
    pub fn set_person_admin(&mut self, person: u32, admin: bool) -> Result<(), Error> {
        validate::person(person, self)?;
        self.persons[person as usize].admin = admin;
        Ok(())
    }
    pub fn add_span(&mut self, person: u32, enter: i64, leave: i64) -> Result<Vec<Span>, Error> {
        validate::person(person, self)?;
        validate::span(enter, leave, self)?;
        let span = Span { enter, leave };
        let min = self
            .person(person)?
            .spans
            .partition_point(|s| s.leave <= enter);
        let max = self
            .person(person)?
            .spans
            .partition_point(|s| s.enter < leave);
        let removed = self.persons[person as usize]
            .spans
            .drain(min..max)
            .collect();
        self.persons[person as usize].spans.insert(min, span);
        Ok(removed)
    }
    pub fn validate_entries(&self) -> Result<(), Error> {
        for (_, person) in self.persons.iter() {
            for &span in person.spans.iter() {
                if span.enter >= span.leave {
                    return Err(Error::InconsistentEntry(span));
                }
            }
            for window in person.spans.windows(2) {
                let &[a, b] = window else { unreachable!() };
                if a.leave >= b.enter {
                    return Err(Error::InconsistentEntry(b));
                }
            }
        }
        Ok(())
    }
    pub fn at(&self, person: u32, instant: i64) -> Result<Result<usize, usize>, Error> {
        Ok(self
            .person(person)?
            .spans
            .binary_search_by(|elem| match () {
                () if elem.leave < instant => Ordering::Less,
                () if elem.enter >= instant => Ordering::Greater,
                () => Ordering::Equal,
            }))
    }
    pub fn is_active(&self, person: u32, instant: i64) -> Result<bool, Error> {
        Ok(match self.at(person, instant)? {
            Ok(..) => true,
            Err(..) => false,
        })
    }
    pub fn select(&self, person: u32, range: Range<i64>) -> Result<Vec<DaySpan>, Error> {
        validate::person(person, self)?;
        let mut spans = Vec::new();
        for span in self.entries(person, range)? {
            spans.extend(self.time_zone.days(span.enter..span.leave));
        }
        Ok(spans)
    }

    pub fn entries(
        &self,
        person: u32,
        range: Range<i64>,
    ) -> Result<impl Iterator<Item = Span>, Error> {
        let start = range.start;
        let end = range.end;
        let min = self
            .person(person)?
            .spans
            .partition_point(|s| s.leave <= range.start);
        let max = self
            .person(person)?
            .spans
            .partition_point(|s| s.enter < range.end);
        Ok(self.person(person)?.spans[min..max]
            .iter()
            .filter_map(move |span| span.conjunction(start..end)))
    }
    pub fn persons(&self) -> impl Iterator<Item = u32> {
        self.persons.iter().map(|(k, _)| k as u32)
    }
}

impl Span {
    fn conjunction(self, range: Range<i64>) -> Option<Self> {
        let selected = Self {
            enter: self.enter.max(range.start),
            leave: self.leave.min(range.end),
        };
        (selected.leave > selected.enter).then_some(selected)
    }
}

impl PartialEq for State {
    fn eq(&self, other: &Self) -> bool {
        if self.persons.len() != other.persons.len() {
            return false;
        }
        for (a, b) in Iterator::zip(self.persons.iter(), other.persons.iter()) {
            if a != b {
                return false;
            }
        }
        true
    }
}
impl Eq for State {}
impl Default for State {
    fn default() -> Self {
        Self {
            time_zone: Tz::UTC,
            persons: [(
                0,
                Person {
                    names: ["admin".to_string()].into(),
                    admin: true,
                    spans: Vec::new(),
                    entered: None,
                },
            )]
            .into_iter()
            .collect(),
        }
    }
}
