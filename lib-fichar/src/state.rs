use super::{Direction, Entry, Error, Person, validate};
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
    entries: Vec<Entry>,
}

impl State {
    pub fn person(&self, person: u32) -> Result<&Person, Error> {
        self.persons
            .get(person as usize)
            .ok_or(Error::InvalidPerson(person))
    }
    pub fn new_person(&mut self, name: String) -> u32 {
        self.persons.insert(Person { name }) as u32
    }
    pub fn set_person_name(&mut self, person: u32, name: String) {
        validate::person(person, self).unwrap();
        self.persons[person as usize].name = name;
    }
    pub fn add_entry(
        &mut self,
        person: u32,
        direction: Direction,
        instant: i64,
    ) -> Result<(), Error> {
        validate::person(person, self)?;
        let entry = Entry {
            person,
            direction,
            instant,
        };
        match self.entries.binary_search_by_key(&entry.key(), Entry::key) {
            Ok(found) => Err(Error::InconsistentEntry(self.entries[found])),
            Err(insertion) => {
                if direction
                    == self
                        .last_direction(person, instant)
                        .unwrap_or(Direction::Leaves)
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
            .binary_search_by_key(&(instant, person), Entry::key)
        {
            Ok(found) => Some(self.entries[found].direction),
            Err(between) => self.entries[..between]
                .iter()
                .rev()
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

    pub fn entries(&self, range: Range<i64>) -> &[Entry] {
        let start = self.entries.partition_point(|e| e.instant < range.start);
        let end = self.entries.partition_point(|e| e.instant < range.end);
        &self.entries[start..end]
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
                    name: "admin".to_string(),
                },
            )]
            .into_iter()
            .collect(),
            entries: Vec::new(),
        }
    }
}
