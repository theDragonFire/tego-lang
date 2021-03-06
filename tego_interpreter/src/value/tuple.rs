use crate::value::Value;
use owned_chars::OwnedCharsExt;
use std::fmt;
use std::iter;

#[derive(Debug, Clone)]
pub enum Tuple {
    Generic(Vec<Value>),
    String(String),
}

impl Tuple {
    pub fn len(&self) -> usize {
        match self {
            Self::Generic(vec) => vec.len(),
            Self::String(string) => string.len(),
        }
    }

    pub fn index(&self, index: usize) -> Value {
        match self {
            Self::Generic(vec) => vec[index].clone(),
            Self::String(string) => Value::Char(string.chars().nth(index).unwrap()),
        }
    }

    pub fn from(&self, from: usize) -> Tuple {
        match self {
            Self::Generic(vec) => vec[from..].into(),
            Self::String(string) => string.chars().skip(from).collect::<String>().into(),
        }
    }

    pub fn append(self, other: Self) -> Self {
        match (self, other) {
            (Self::String(mut str1), Self::String(str2)) => {
                str1.push_str(&str2);
                str1.into()
            }
            (Self::String(str1), Self::Generic(tup2)) => str1
                .chars()
                .map(Value::Char)
                .chain(tup2.into_iter())
                .collect(),
            (Self::Generic(tup1), Self::String(str2)) => tup1
                .into_iter()
                .chain(str2.chars().map(Value::Char))
                .collect(),
            (Self::Generic(tup1), Self::Generic(tup2)) => {
                tup1.into_iter().chain(tup2.into_iter()).collect()
            }
        }
    }

    pub fn push_to_front(self, value: Value) -> Self {
        match (self, value) {
            (Self::String(mut s), Value::Char(c)) => {
                s.insert(0, c);
                s.into()
            }
            (Self::String(s), v) => iter::once(v).chain(s.chars().map(Value::Char)).collect(),
            (Self::Generic(mut vec), v) => {
                vec.insert(0, v);
                vec.into()
            }
        }
    }

    pub fn push_to_back(self, value: Value) -> Self {
        match (self, value) {
            (Self::String(mut s), Value::Char(c)) => {
                s.push(c);
                s.into()
            }
            (Self::String(s), v) => s.chars().map(Value::Char).chain(iter::once(v)).collect(),
            (Self::Generic(mut vec), v) => {
                vec.push(v);
                vec.into()
            }
        }
    }

    pub fn is_unit(&self) -> bool {
        self.len() == 0
    }

    pub fn get(&self, index: usize) -> Value {
        match self {
            Self::Generic(vec) => vec.get(index).unwrap_or(&Value::unit()).clone(),
            Self::String(string) => string
                .chars()
                .map(Value::Char)
                .nth(index)
                .unwrap_or_else(Value::unit),
        }
    }
}

impl PartialEq for Tuple {
    fn eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            false
        } else {
            match (self, other) {
                (Self::Generic(a), Self::Generic(b)) => a == b,
                (Self::String(a), Self::String(b)) => a == b,
                (a, b) => a.into_iter().eq(b.into_iter()),
            }
        }
    }
}

conversion!( Tuple[vec: Vec<Value>] => Tuple::Generic(vec));
conversion!( Tuple[slice: &[Value]] => Tuple::Generic(slice.into()));
conversion!( Tuple[string: String] => Tuple::String(string));
conversion!( Tuple[string: &str] => Tuple::String(string.into()));

impl IntoIterator for Tuple {
    type Item = Value;
    type IntoIter = TupleIter;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            Tuple::Generic(tuple) => TupleIter::new(Box::new(tuple.into_iter())),
            Tuple::String(string) => TupleIter::new(Box::new(string.into_chars().map(Value::Char))),
        }
    }
}

impl iter::FromIterator<Value> for Tuple {
    fn from_iter<I: IntoIterator<Item = Value>>(iter: I) -> Self {
        iter.into_iter().collect::<Vec<_>>().into()
    }
}

impl iter::FromIterator<char> for Tuple {
    fn from_iter<I: IntoIterator<Item = char>>(iter: I) -> Self {
        iter.into_iter().collect::<String>().into()
    }
}

impl IntoIterator for &Tuple {
    type Item = Value;
    type IntoIter = TupleIter;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            Tuple::Generic(tuple) => TupleIter::new(Box::new(tuple.clone().into_iter())),
            Tuple::String(string) => {
                TupleIter::new(Box::new(string.clone().into_chars().map(Value::Char)))
            }
        }
    }
}

pub struct TupleIter {
    iter: Box<dyn Iterator<Item = Value>>,
}

impl TupleIter {
    fn new(iter: Box<dyn Iterator<Item = Value>>) -> Self {
        TupleIter { iter }
    }
}

impl Iterator for TupleIter {
    type Item = Value;
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

impl std::fmt::Display for Tuple {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> fmt::Result {
        match self {
            Self::Generic(vec) => {
                let result = vec
                    .iter()
                    .map(|v| format!("{}", v))
                    .fold(String::new(), |a, s| a + &s + ", ");

                let result = if result.len() >= 2 {
                    &result[..result.len() - 2] // Get rid of the last ", "
                } else {
                    &result
                };

                write!(f, "({})", result)
            }
            Self::String(string) => write!(f, "\"{}\"", string),
        }
    }
}
