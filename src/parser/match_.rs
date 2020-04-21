use nom::branch::alt;
use crate::parser::tokens::*;
use crate::ast::Match;

use nom::{
    IResult,
    sequence::{
        pair,
        terminated
    },
    combinator::opt,
    error::ErrorKind
};

type MatchResult<'a> = IResult<&'a str, Match>;

pub fn match_(input: &'_ str) -> MatchResult<'_> {
    tuple(input)
}

pub fn tuple(input: &'_ str) -> MatchResult<'_> {
    pair(grouping, opt(comma))(input).and_then(
        |(input, (a, comma))|
        match comma {
            Some(_) => tuple(input).map(
                |(input, b)|
                (input, Match::tuple(a, b))
            ),
            None => Ok((input, a))
        }
    )
}

pub fn grouping(input: &'_ str) -> MatchResult<'_> {
    opt(left_paren)(input).and_then(
        |(input, left_paren)|
        match left_paren {
            Some(_) => terminated(match_, right_paren)(input),
            None => atom(input)
        }
    )
}

pub fn atom(input: &'_ str) -> MatchResult<'_> {
    alt((true_val, false_val, number, identifier))(input).and_then(
        |(new_input, token)|
        match token {
            "true" => Ok((new_input, Match::bool(true))),
            "false" => Ok((new_input, Match::bool(false))),
            lexeme if is_keyword(lexeme) =>
                Err(nom::Err::Error((input, ErrorKind::Tag))),
            lexeme => if let Ok(i) = lexeme.parse::<i32>() {
                    Ok((new_input, Match::int(i)))
                } else {
                    Ok((new_input, Match::ident(lexeme)))
                }
        }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    
    parser_test! {
        ident_test
        (match_): "abc" => Match::ident("abc")
    }
    
    parser_test! {
        tuple_test
        (match_): "a, b" =>
            Match::tuple(
                Match::ident("a"),
                Match::ident("b")
            );
        (match_): "a, b, c" =>
            Match::tuple(
                Match::tuple(
                    Match::ident("a"),
                    Match::ident("b")
                ), Match::ident("c")
            )
    }
    
    // Not actually super effective, the order
    // of tuples doesn't really matter
    parser_test! {
        grouping_test
        (match_): "a, (b , c)" =>
            Match::tuple(
                Match::ident("a"),
                Match::tuple(
                    Match::ident("b"),
                    Match::ident("c")
                )
            )
    }
    
    parser_test! {
        value_test
        (match_): "1" =>
            Match::int(1);
        (match_): "true" =>
            Match::bool(true)
    }
    
    basic_test! {
        keyword
        match_("let") =>
            Err(nom::Err::Error(("let", ErrorKind::Tag)))
    }
}