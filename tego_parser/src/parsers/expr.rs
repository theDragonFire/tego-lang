use crate::error::*;
use crate::parsers::match_::*;
use crate::parsers::tokens::*;
use crate::{ExprOutput, MatchOutput};
use crate::Input;
use crate::ParseResult;

use nom::{
    branch::alt,
    combinator::opt,
    multi::{fold_many0, many1},
    sequence::{pair, preceded, separated_pair, terminated, tuple},
};

type ExprResult<'a, E> = ParseResult<'a, E>;

macro_rules! binary_expr {
    ($name:ident, $op_func:expr, $next_precedence:ident) => {
        fn $name<E>(input: Input<'_>) -> ExprResult<'_, E>
        where
            E: ExprOutput,
        {
            pair(
                $next_precedence,
                opt(many1(pair(opt_nl($op_func), $next_precedence))),
            )(input)
            .and_then(|(input, (a, other))| match other {
                // Operators found (left to right)
                Some(others) => Ok((
                    input,
                    others
                        .into_iter()
                        .fold(a, |a, (op, b)| E::binary(a, op.to_str(), b)),
                )),
                // No operators found
                None => Ok((input, a)),
            })
        }
    };
}

macro_rules! unary_expr {
    ($name:ident, $op_func:expr, $next_precedence:ident) => {
        fn $name<E>(input: Input<'_>) -> ExprResult<'_, E>
        where
            E: ExprOutput,
        {
            pair($op_func, $name)(input)
                .and_then(|(input, (op, a))| Ok((input, E::unary(op.to_str(), a))))
                .or_else(try_parser($next_precedence, input))
        }
    };
}

pub fn expr<E>(input: Input<'_>) -> ExprResult<'_, E>
where
    E: ExprOutput,
{
    do_expr(input)
}

pub fn do_expr<E>(input: Input<'_>) -> ExprResult<'_, E>
where
    E: ExprOutput,
{
    do_(input)
        .and_then(|(input, _)|
            tuple((let_expr, opt(preceded(in_, match_)), opt_nl(then), expr))(input)
            .map_err(do_error)
            .map(|(input, (command, command_match, _, body))|
                (input, E::do_expr(command, command_match.unwrap_or_else(E::Match::ignore), body))
            )
        )
        .or_else(try_parser(let_expr, input))
}

pub fn let_expr<E>(input: Input<'_>) -> ExprResult<'_, E>
where
    E: ExprOutput,
{
    alt((let_, delay))(input)
        .and_then(|(input, let_token)| match let_token.into() {
            "let" => {
                separated_pair(separated_pair(match_, assign, if_expr), opt_nl(in_), expr)(input)
                    .map_err(let_assign_error)
                    .map(|(input, ((ident, value), inner))| {
                        (input, E::let_expr(ident, value, inner))
                    })
            }
            "delay" => separated_pair(
                separated_pair(variable, assign, join_expr),
                opt_nl(in_),
                expr,
            )(input)
            .map_err(delay_assign_error)
            .map(|(input, ((ident, value), inner))| (input, E::delayed(ident, value, inner))),
            _ => unreachable!(),
        })
        .or_else(try_parser(if_expr, input))
}

pub fn if_expr<E>(input: Input<'_>) -> ExprResult<'_, E>
where
    E: ExprOutput,
{
    if_(input)
        .and_then(|(input, _)| {
            terminated(join_expr, opt_nl(alt((then, q_mark))))(input)
                .map_err(if_cond_error)
                .and_then(|(input, cond)| {
                    separated_pair(opt_nl(expr), opt_nl(else_), expr)(input)
                        .map_err(if_body_error)
                        .map(|(input, (t, f))| (input, E::if_expr(cond, t, f)))
                })
        })
        .or_else(try_parser(match_expr, input))
}

pub fn match_expr<E>(input: Input<'_>) -> ExprResult<'_, E>
where
    E: ExprOutput,
{
    match_kw(input)
        .and_then(|(input, _)| {
            terminated(join_expr, to)(input)
                .map_err(match_head_error)
                .and_then(|(input, val)| {
                    // nl has to be preceding so as not to conflict with
                    // the `req_nl` parser that likely directly follows the match expr
                    many1(preceding_opt_nl(match_arm))(input)
                        .and_then(|(input, patterns)| Ok((input, E::match_(val, patterns))))
                })
        })
        .or_else(try_parser(join_expr, input))
}

pub fn match_arm<E>(input: Input<'_>) -> ParseResult<'_, (E::Match, E)>
where
    E: ExprOutput,
{
    preceded(bar, separated_pair(match_, opt_nl(arrow), expr))(input).map_err(match_arm_error)
}

binary_expr!(join_expr, comma, flat_join_expr);
binary_expr!(flat_join_expr, double_comma, or_expr);
binary_expr!(or_expr, or, xor_expr);
binary_expr!(xor_expr, xor, and_expr);
binary_expr!(and_expr, and, equal_expr);
binary_expr!(equal_expr, alt((equal, not_equal)), compare_expr);
binary_expr!(
    compare_expr,
    alt((less_than_equal, greater_than_equal, less_than, greater_than)),
    add_expr
);
binary_expr!(add_expr, alt((plus, minus)), mult_expr);
binary_expr!(mult_expr, alt((star, slash, modulo)), negate_expr);

unary_expr!(negate_expr, minus, not_expr);
unary_expr!(not_expr, not, fn_expr);

fn fn_expr<E>(input: Input<'_>) -> ExprResult<'_, E>
where
    E: ExprOutput,
{
    fn_(input)
        .and_then(|(input, _)| {
            separated_pair(match_, opt_nl(arrow), expr)(input)
                .map_err(fn_expr_error)
                .map(|(input, (param, body))| (input, E::fn_expr(param, body)))
        })
        .or_else(try_parser(fn_application, input))
}

fn fn_application<E>(input: Input<'_>) -> ExprResult<'_, E>
where
    E: ExprOutput,
{
    dot_expr(input).and_then(|(input, val)| fold_many0(dot_expr, val, E::fn_app)(input))
}

fn dot_expr<E>(input: Input<'_>) -> ExprResult<'_, E>
where
    E: ExprOutput,
{
    pair(
        grouping,
        opt(many1(pair(opt_nl(dot), fn_application))),
    )(input)
    .and_then(|(input, (a, other))| match other {
        // Operators found (left to right)
        Some(others) => Ok((
            input,
            others
                .into_iter()
                .fold(a, |a, (_, b)| E::fn_app(b, a)),
        )),
        // No operators found
        None => Ok((input, a)),
    })
}

fn grouping<E>(input: Input<'_>) -> ExprResult<'_, E>
where
    E: ExprOutput,
{
    opt_nl(left_paren)(input)
        .and_then(|(input, open_paren)| {
            right_paren(input)
                .map(|(input, _)| (input, E::unit()))
                .or_else(try_parser(terminated(opt_nl(expr), right_paren), input))
                .map_err(terminating_paren_error((
                    open_paren.line(),
                    open_paren.column(),
                )))
        })
        .or_else(try_parser(
            |input| {
                opt_nl(left_bracket)(input).and_then(|(input, open_bracket)| {
                    terminated(opt_nl(expr), right_bracket)(input)
                        .map(|(input, inner)| (input, E::boxed(inner)))
                        .map_err(terminating_bracket_error((
                            open_bracket.line(),
                            open_bracket.column(),
                        )))
                })
            },
            input,
        ))
        .or_else(try_parser(literal, input))
}

fn literal<E>(input: Input<'_>) -> ExprResult<'_, E>
where
    E: ExprOutput,
{
    alt((true_val, false_val, number, identifier))(input)
        .and_then(|(new_input, token)| match token.to_str() {
            "true" => Ok((new_input, E::bool(true))),
            "false" => Ok((new_input, E::bool(false))),
            lexeme => {
                if let Ok(i) = lexeme.parse::<i32>() {
                    Ok((new_input, E::int(i)))
                } else {
                    Ok((new_input, E::variable(lexeme)))
                }
            } // Has to be done seperately so that it doesn't get mixed up as an identifier
        })
        .or_else(|_| string(input).map(|(input, s)| (input, E::string(s.into()))))
        .or_else(|_| char(input).map(|(input, c)| (input, E::char(c))))
        .map_err(literal_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Expr, Match};
    use crate::test::*;

    parser_test! {
        literal_test
        (expr): "1" => Expr::int(1);
        (expr): "true" => Expr::bool(true);
        (expr): "false" => Expr::bool(false);
        (expr): "()" => Expr::unit()
    }
    parser_test! {
        or_test
        (expr): "true or\nfalse" =>
            Expr::or(
                Expr::bool(true),
                Expr::bool(false))
    }
    parser_test! {
        and_test
        (expr): "true and\nfalse" =>
            Expr::and(
                Expr::bool(true),
                Expr::bool(false))
    }
    parser_test! {
        plus_test
        (expr): "1 +\n2" =>
            Expr::plus(
                Expr::int(1),
                Expr::int(2))
    }
    parser_test! {
        minus_test
        (expr): "1 -\n2" =>
            Expr::minus(
                Expr::int(1),
                Expr::int(2))
    }
    parser_test! {
        multiply_test
        (expr): "1 *\n2" =>
            Expr::multiply(
                Expr::int(1),
                Expr::int(2))
    }
    parser_test! {
        divide_test
        (expr): "1 /\n2" =>
            Expr::divide(
                Expr::int(1),
                Expr::int(2))
    }
    parser_test! {
        modulo_test
        (expr): "1 %\n2" =>
            Expr::modulo(
                Expr::int(1),
                Expr::int(2))
    }
    parser_test! {
        equal_test
        (expr): "1 ==\n2" =>
            Expr::equal(
                Expr::int(1),
                Expr::int(2))
    }
    parser_test! {
        negate_test
        (expr): "-1" =>
            Expr::negate(
                Expr::int(1))
    }
    parser_test! {
        not_test
        (expr): "not true" =>
            Expr::not(
                Expr::bool(true))
    }
    parser_test! {
        grouping_test
        (expr): "1 + 2 * 3" =>
            Expr::plus(
                Expr::int(1),
                Expr::multiply(
                    Expr::int(2),
                    Expr::int(3)));
        (expr): "(1 + 2) * 3" =>
            Expr::multiply(
                Expr::plus(
                    Expr::int(1),
                    Expr::int(2)),
                Expr::int(3))
    }
    parser_test! {
        precedence_test
        (expr): "1 or 2 and 3 == 4 + 5 - 6 * 7 / 8 % 9" =>
            Expr::or(
                Expr::int(1),
                Expr::and(
                    Expr::int(2),
                    Expr::equal(
                        Expr::int(3),
                        Expr::minus(
                            Expr::plus(
                                Expr::int(4),
                                Expr::int(5)),
                            Expr::modulo(
                                Expr::divide(
                                    Expr::multiply(
                                        Expr::int(6),
                                        Expr::int(7)),
                                    Expr::int(8))
                                , Expr::int(9))))));
        (expr): "1 and 2 and 3" =>
            Expr::and(
                Expr::and(
                    Expr::int(1),
                    Expr::int(2)),
                Expr::int(3));
        (expr): "1 == 2 /= 3 < 4 > 5 <= 6 >= 7 and 8" =>
            Expr::and(
                Expr::not_equal(
                    Expr::equal(
                        Expr::int(1),
                        Expr::int(2)),
                    Expr::greater_than_equal(
                        Expr::less_than_equal(
                            Expr::greater_than(
                                Expr::less_than(
                                    Expr::int(3), Expr::int(4)),
                                Expr::int(5)),
                            Expr::int(6)),
                        Expr::int(7))),
                Expr::int(8))
    }
    parser_test! {
        if_else_test
        (expr): "if true then\n1\nelse\n2" =>
            Expr::if_expr(
                Expr::bool(true),
                Expr::int(1),
                Expr::int(2));
        (expr): "if true ?\n1\nelse\n2" =>
            Expr::if_expr(
                Expr::bool(true),
                Expr::int(1),
                Expr::int(2))
    }
    parser_test! {
        variable_test
        (expr): "abc" =>
            Expr::variable("abc")
    }
    parser_test! {
        let_test
        (expr): "let a = 1 in\ntrue" =>
            Expr::let_expr(
                Match::ident("a"),
                Expr::int(1),
                Expr::bool(true));
        (expr): "let a = 1 in\n2" =>
            Expr::let_expr(
                Match::ident("a"),
                Expr::int(1),
                Expr::int(2)
            )

    }
    parser_test! {
        fn_test
        (expr): "fn a ->\na" =>
            Expr::fn_expr(
                Match::ident("a"),
                Expr::variable("a"));
        (expr): "fn a ->\na + 1" =>
            Expr::fn_expr(
                Match::ident("a"),
                Expr::plus(
                    Expr::variable("a"),
                    Expr::int(1)))
    }
    parser_test! {
        fn_application_test
        (expr): "a 1" =>
            Expr::fn_app(
                Expr::variable("a"),
                Expr::int(1));
        (expr): "a (\n1, 2\n)" =>
            Expr::fn_app(
                Expr::variable("a"),
                Expr::join(
                    Expr::int(1),
                    Expr::int(2)
                )
            );
        (expr): "a 1 2" =>
            Expr::fn_app(
                Expr::fn_app(
                    Expr::variable("a"),
                    Expr::int(1)),
                Expr::int(2))
    }
    parser_test! {
        match_expr_test
        (expr): "match 1 to\n| 1 -> true\n| a -> false" =>
            Expr::match_(Expr::int(1), vec![
                (Match::int(1), Expr::bool(true)),
                (Match::ident("a"), Expr::bool(false))
            ])
    }
    parser_test! {
        delayed_value_test
        (expr): "delay a = 1 in a" =>
            Expr::delayed(
                Match::ident("a"),
                Expr::int(1),
                Expr::variable("a")
            )
    }
    parser_test! {
        do_expr_test
        (expr): "do println 1 in a then b" =>
            Expr::do_expr(
                Expr::fn_app(Expr::variable("println"), Expr::int(1)),
                Match::ident("a"),
                Expr::variable("b")
            );
        (expr): "do println 1 then a" =>
            Expr::do_expr(
                Expr::fn_app(Expr::variable("println"), Expr::int(1)), 
                Match::ignore(),
                Expr::variable("a")
            )
    }
    parser_test! {
        string_test
        (expr): "\"abc\"" => Expr::string("abc")
    }
    parser_test! {
        char_test
        (expr): "'a'" => Expr::char('a')
    }
    parser_test! {
        boxed_tuple_test
        (expr): "[1, 2]" => Expr::boxed(Expr::join(Expr::int(1), Expr::int(2)))
    }
    parser_test! {
        flat_join_test
        (expr): "1 ,, 2" => Expr::flat_join(Expr::int(1), Expr::int(2))
    }
}
