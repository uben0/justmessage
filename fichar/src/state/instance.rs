use crate::language::Language;
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, ops::Range};
use time_util::TimeZoneExt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instance {
    pub language: Language,
    pub time_zone: Tz,
    persons: HashMap<i64, Person>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Person {
    spans: Vec<Span>,
    entered: Option<i64>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Span {
    pub enter: i64,
    pub leave: i64,
}

impl Instance {
    pub fn new_spain() -> Self {
        Self::new(Language::Es, Tz::Europe__Madrid)
    }
    pub fn new(language: Language, time_zone: Tz) -> Self {
        Self {
            language,
            time_zone,
            persons: HashMap::new(),
        }
    }
    pub fn get_name(&self, person: i64) -> Option<String> {
        let person = self.person(person)?;
        let mut names = Vec::new();
        if let Some(ref first_name) = person.first_name {
            names.push(first_name.as_str());
        }
        if let Some(ref last_name) = person.last_name {
            names.push(last_name.as_str());
        }
        if names.is_empty() {
            return None;
        } else {
            Some(names.join(" "))
        }
    }
    pub fn set_first_name(&mut self, person: i64, first_name: String) {
        self.persons.entry(person).or_default().first_name = Some(first_name);
    }
    pub fn set_last_name(&mut self, person: i64, last_name: String) {
        self.persons.entry(person).or_default().last_name = Some(last_name);
    }
    pub fn with_person(&mut self, person: i64) -> &mut Self {
        self.persons.entry(person).or_default();
        self
    }
    pub fn person(&self, person: i64) -> Option<&Person> {
        self.persons.get(&person)
    }
    pub fn remove_person(&mut self, person: i64) {
        self.persons.remove(&person);
    }
    pub fn add_span(
        &mut self,
        person: i64,
        enter: i64,
        leave: i64,
    ) -> Result<Vec<Span>, AddSpanError> {
        let span = Span { enter, leave };
        if span.enter >= span.leave {
            return Err(AddSpanError::LeaveEarlierThanEnter(span));
        }
        let person = self.persons.entry(person).or_insert(Person::default());
        let min = person.spans.partition_point(|s| s.leave <= enter);
        let max = person.spans.partition_point(|s| s.enter < leave);
        let removed = person.spans.drain(min..max).collect();
        person.spans.insert(min, span);
        Ok(removed)
    }
    pub fn enter(&mut self, person: i64, enter: i64) -> Option<i64> {
        let person = self.persons.entry(person).or_insert(Person::default());
        person.entered.replace(enter)
    }
    pub fn leave(&mut self, person: i64, leave: i64) -> Result<(Span, Vec<Span>), LeaveError> {
        let Some(person_obj) = self.persons.get_mut(&person) else {
            return Err(LeaveError::NotEntered);
        };
        let Some(enter) = person_obj.entered.take() else {
            return Err(LeaveError::NotEntered);
        };
        match self.add_span(person, enter, leave) {
            Ok(overriden) => Ok((Span { enter, leave }, overriden)),
            Err(AddSpanError::LeaveEarlierThanEnter(span)) => {
                Err(LeaveError::LeaveEarlierThanEnter(span))
            }
        }
    }
    pub fn entered(&self, person: i64) -> Option<i64> {
        self.persons.get(&person)?.entered
    }
    pub fn entries(&self, person: i64, start: i64, end: i64) -> impl Iterator<Item = Span> {
        let slice = if let Some(person) = self.persons.get(&person) {
            let min = person.spans.partition_point(|s| s.leave <= start);
            let max = person.spans.partition_point(|s| s.enter < end);
            &person.spans[min..max]
        } else {
            &[]
        };
        slice
            .iter()
            .filter_map(move |span| span.conjunction(start..end))
    }
    pub fn clear(&mut self, person: i64, start: i64, end: i64) -> Vec<Span> {
        if let Some(person) = self.persons.get_mut(&person) {
            let min = person.spans.partition_point(|s| s.leave <= start);
            let max = person.spans.partition_point(|s| s.enter < end);
            person.spans.drain(min..max).collect()
        } else {
            Vec::new()
        }
    }
    pub fn select(&self, person: i64, start: i64, end: i64) -> Vec<Span> {
        let mut spans = Vec::new();
        for span in self.entries(person, start, end) {
            spans.extend(
                self.time_zone
                    .split_span_on_day(span.enter..span.leave)
                    .map(|range| Span {
                        enter: range.start,
                        leave: range.end,
                    }),
            );
        }
        spans
    }
    pub fn persons(&self) -> impl Iterator<Item = i64> {
        self.persons.keys().copied()
    }
}

pub enum AddSpanError {
    LeaveEarlierThanEnter(Span),
}
pub enum LeaveError {
    NotEntered,
    LeaveEarlierThanEnter(Span),
}

impl Span {
    fn conjunction(self, range: Range<i64>) -> Option<Self> {
        let selected = Self {
            enter: self.enter.max(range.start),
            leave: self.leave.min(range.end),
        };
        (selected.leave > selected.enter).then_some(selected)
    }
    pub fn minutes(self) -> u32 {
        (self.leave - self.enter) as u32 / 60
    }
}
