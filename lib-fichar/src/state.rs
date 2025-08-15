use super::{Direction, Error, Person, Span, validate};
use chrono::{Months, TimeZone};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use slab::Slab;
use std::{collections::HashMap, ops::Range};
use time_util::{DaySpan, TimeZoneExt};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct State {
    pub time_zone: Tz,
    persons: Slab<Person>,
    entries: Vec<Span>,
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
        }) as u32
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
    pub fn add_entry(
        &mut self,
        person: u32,
        direction: Direction,
        instant: i64,
    ) -> Result<(), Error> {
        validate::person(person, self)?;
        let entry = Span {
            person,
            direction,
            instant,
        };
        match self.entries.binary_search_by_key(&entry.key(), Span::key) {
            Ok(found) => Err(Error::InconsistentEntry(self.entries[found])),
            Err(insertion) => {
                if direction
                    == self
                        .last_direction(person, instant)
                        .unwrap_or(Direction::Leaves)
                {
                    Err(Error::InconsistentEntry(entry))
                } else if let Some(first) = self.first_direction(person, instant)
                    && first == direction
                {
                    Err(Error::InconsistentEntry(entry))
                } else {
                    self.entries.insert(insertion, entry);
                    Ok(())
                }
            }
        }
    }
    pub fn validate_entries(&self) -> Result<(), Error> {
        for window in self.entries.windows(2) {
            let &[a, b] = window else { unreachable!() };
            if a.key() >= b.key() {
                return Err(Error::InconsistentEntry(b));
            }
        }

        let mut active = HashMap::new();
        for &entry in &self.entries {
            validate::person(entry.person, self)?;
            let active = active.entry(entry.person).or_insert(false);
            let new = match entry.direction {
                Direction::Enters => true,
                Direction::Leaves => false,
            };
            if *active == new {
                return Err(Error::InconsistentEntry(entry));
            }
            *active = new;
        }

        Ok(())
    }
    pub fn last_direction(&self, person: u32, instant: i64) -> Option<Direction> {
        validate::person(person, self).unwrap();
        match self
            .entries
            .binary_search_by_key(&(instant, person), Span::key)
        {
            Ok(found) => Some(self.entries[found].direction),
            Err(between) => self.entries[..between]
                .iter()
                .rev()
                .find(|e| e.person == person)
                .map(|e| e.direction),
        }
    }
    pub fn first_direction(&self, person: u32, instant: i64) -> Option<Direction> {
        validate::person(person, self).unwrap();
        match self
            .entries
            .binary_search_by_key(&(instant, person), Span::key)
        {
            Ok(found) => Some(self.entries[found].direction),
            Err(between) => self.entries[between..]
                .iter()
                .find(|e| e.person == person)
                .map(|e| e.direction),
        }
    }
    pub fn is_active(&self, person: u32, instant: i64) -> bool {
        validate::person(person, self).unwrap();
        match self
            .last_direction(person, instant)
            .unwrap_or(Direction::Leaves)
        {
            Direction::Enters => true,
            Direction::Leaves => false,
        }
    }
    pub fn select(&self, person: u32, range: Range<i64>) -> Result<Vec<DaySpan>, Error> {
        validate::person(person, self)?;

        let mut active = self.is_active(person, range.start).then_some(range.start);
        let mut spans = Vec::new();
        for &entry in self.entries(range.clone()) {
            match (entry.direction, active.take()) {
                (Direction::Enters, None) => {
                    active = Some(entry.instant);
                }
                (Direction::Leaves, Some(start)) => {
                    spans.extend(self.time_zone.days(start..entry.instant));
                }
                (_, _) => return Err(Error::InconsistentEntry(entry)),
            }
        }
        if let Some(start) = active.take() {
            spans.extend(self.time_zone.days(start..range.end));
        }
        Ok(spans)
    }
    pub fn select_month(&self, person: u32, year: i32, month: u32) -> Result<Vec<DaySpan>, Error> {
        validate::month(month)?;
        validate::person(person, self)?;

        let range = {
            let start = self
                .time_zone
                .with_ymd_and_hms(year, month, 1, 0, 0, 0)
                .unwrap();
            let end = start + Months::new(1);
            start.timestamp()..end.timestamp()
        };
        let mut active = self.is_active(person, range.start).then_some(range.start);
        let mut spans = Vec::new();
        for &entry in self.entries(range.clone()) {
            match (entry.direction, active.take()) {
                (Direction::Enters, None) => {
                    active = Some(entry.instant);
                }
                (Direction::Leaves, Some(start)) => {
                    spans.extend(self.time_zone.days(start..entry.instant));
                }
                (_, _) => return Err(Error::InconsistentEntry(entry)),
            }
        }
        if let Some(start) = active.take() {
            spans.extend(self.time_zone.days(start..range.end));
        }
        Ok(spans)
    }

    pub fn entries(&self, range: Range<i64>) -> &[Span] {
        let start = self.entries.partition_point(|e| e.instant < range.start);
        let end = self.entries.partition_point(|e| e.instant < range.end);
        &self.entries[start..end]
    }
    pub fn persons(&self) -> impl Iterator<Item = u32> {
        self.persons.iter().map(|(k, _)| k as u32)
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
        self.entries == self.entries
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
                },
            )]
            .into_iter()
            .collect(),
            entries: Vec::new(),
        }
    }
}
