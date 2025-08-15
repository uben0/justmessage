use pest::{Parser, error::Error, iterators::Pair};
use pest_derive::Parser;
use std::{collections::HashMap, str::FromStr};

#[derive(Parser, Debug, Clone, PartialEq, Eq)]
#[grammar = "grammar.pest"]
pub enum Json {
    Null,
    Bool(bool),
    Int(i64),
    String(String),
    Array(Vec<Self>),
    Object(HashMap<String, Self>),
}

impl<'a> From<&'a str> for Json {
    fn from(value: &'a str) -> Self {
        Self::String(value.to_string())
    }
}
impl From<i64> for Json {
    fn from(value: i64) -> Self {
        Self::Int(value)
    }
}
impl From<i32> for Json {
    fn from(value: i32) -> Self {
        Self::Int(value as i64)
    }
}
impl From<u32> for Json {
    fn from(value: u32) -> Self {
        Self::Int(value as i64)
    }
}

impl FromStr for Json {
    type Err = Error<Rule>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let node = Json::parse(Rule::value, s)?
            .next()
            .unwrap()
            .into_inner()
            .next()
            .unwrap();
        Ok(node.into())
    }
}

impl<'a> From<Pair<'a, Rule>> for Json {
    fn from(node: Pair<'a, Rule>) -> Self {
        match node.as_rule() {
            Rule::null => Self::Null,
            Rule::bool => Self::Bool(match node.as_str() {
                "true" => true,
                "false" => false,
                _ => unreachable!(),
            }),
            Rule::int => Self::Int(node.as_str().parse().unwrap()),
            Rule::array => Self::Array(
                node.into_inner()
                    .map(|v| v.into_inner().next().unwrap().into())
                    .collect(),
            ),
            Rule::string => Self::String(
                node.into_inner()
                    .map(|elem| match elem.as_rule() {
                        Rule::char => elem.as_str().chars().next().unwrap(),
                        Rule::char_bs => '\\',
                        Rule::char_sq => '\'',
                        Rule::char_dq => '\"',
                        Rule::char_ln => '\n',
                        c => unreachable!("found {:?}", c),
                    })
                    .collect(),
            ),
            Rule::object => Self::Object(
                node.into_inner()
                    .map(|attr| {
                        let mut children = attr.into_inner();
                        let key = children.next().unwrap();
                        let value: Self =
                            children.next().unwrap().into_inner().next().unwrap().into();
                        let Self::String(key) = key.into() else {
                            unreachable!()
                        };
                        (key, value)
                    })
                    .collect(),
            ),
            rule => unreachable!("found {:?} for {:?}", rule, node.as_str()),
        }
    }
}

impl Json {
    pub fn str(string: impl Into<String>) -> Json {
        Json::String(string.into())
    }
    pub fn array<const N: usize>(array: [Json; N]) -> Json {
        Json::Array(Vec::from(array))
    }
    pub fn object<const N: usize>(object: [(&str, Json); N]) -> Json {
        let mut map = HashMap::new();
        for (key, value) in object {
            map.insert(key.to_string(), value);
        }
        Json::Object(map)
    }
}

#[test]
fn test_parse() {
    use indoc::indoc;
    for (string, expect) in [
        (
            indoc! {r#"
                null
            "# },
            Json::Null,
        ),
        (
            indoc! {r#"
                43
            "# },
            Json::Int(43),
        ),
        (
            indoc! {r#"
                " "
            "# },
            Json::str(" "),
        ),
        (
            indoc! {r#"
                []
            "# },
            Json::array([]),
        ),
        (
            indoc! {r#"
                [true]
            "# },
            Json::array([Json::Bool(true)]),
        ),
        (
            indoc! {r#"
                { " hello"  :"\""}
            "# },
            Json::object([(" hello", Json::str("\""))]),
        ),
        (
            indoc! {r#"
                {
                    "hello": null,
                    "world": -12
                }
            "# },
            Json::object([("hello", Json::Null), ("world", Json::Int(-12))]),
        ),
    ] {
        let value: Json = match string.parse() {
            Ok(value) => value,
            Err(err) => panic!("{:?}", err),
        };
        assert_eq!(value, expect);
    }
}
