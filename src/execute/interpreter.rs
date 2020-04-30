use crate::ast::expr::Expr;
use crate::ast::decl::Decl;
use crate::environment::{Env, EnvWrapper};
use crate::execute::value::Value;
use std::rc::Rc;

pub type VarEnv = Env<Value>;
pub type WrappedEnv = EnvWrapper<VarEnv>;

pub fn new_env() -> WrappedEnv {
    VarEnv::empty()
}

pub fn env_from_decls(decls: &[Decl]) -> WrappedEnv {
    let (env, decl_ptrs) = unfilled_env(decls);
    fill_decl_env(decls, &decl_ptrs, env)
}

fn unfilled_env(decls: &[Decl]) -> (WrappedEnv, Vec<WrappedEnv>) {
    decls.iter().map(
        |decl|
        match decl {
            Decl::Expression(ident, _) => (ident, Value::Error(
                format!("'{}' has not been initialized", ident)
            ))
        }
    ).fold(
        (new_env(), Vec::with_capacity(decls.len())),
        |(parent, mut decl_ptrs), (ident, val)| {
            let new_env = Env::associate_ident(ident.to_string(), val, parent);
            decl_ptrs.push(Rc::clone(&new_env));
            (new_env, decl_ptrs)
        }
    )
}

fn fill_decl_env(decls: &[Decl], decl_ptrs: &[WrappedEnv], env: WrappedEnv) -> WrappedEnv {
    decls.iter().zip(decl_ptrs.iter()).for_each(
        |(decl, decl_ptr)|
        match decl {
            Decl::Expression(_, Expr::Fn_(param, body)) =>
                Env::set_value(decl_ptr, Value::decl_function(
                    param.clone(),
                    body.clone(),
                    Rc::downgrade(&env)
                )),
            Decl::Expression(_, expr) =>
                Env::set_value(decl_ptr, eval_expr(expr.clone(), &env)) // Need to delay this
//                Value::delayed_decl(
//                    expr,
//                    Rc::downgrade(&env),
//                    &decl_ptr
//                ))
        }
    );
    env
}

pub fn eval_expr(expr: Expr, env: &WrappedEnv) -> Value {
    match expr {
        Expr::Unary(op, a) => op.eval(eval_expr(*a, env)),
        Expr::Binary(a, op, b) =>
            op.eval(
                eval_expr(*a, env),
                eval_expr(*b, env)
            ),
        Expr::Literal(val) => val,
        Expr::If(cond, a, b) => match eval_expr(*cond, env) {
            Value::Bool(true) => eval_expr(*a, env),
            Value::Bool(false) => eval_expr(*b, env),
            _ => error("If condition must return a boolean")
        },
        Expr::Variable(ident) => match Env::get(env, &ident) {
            Some(val) => val.clone(),
            None =>
                error(&format!("Variable '{}' is not declared", ident))
        },
        Expr::Let(ident, value, inner) =>
            match VarEnv::associate(
                ident,
                eval_expr(*value, env),
                env
            ) {
                Ok(env) => eval_expr(*inner, &env),
                Err(error) => Value::Error(error)
            },
        Expr::Fn_(param, body) =>
            Value::function(param, body, Rc::clone(env)),
        Expr::FnApp(function, arg) => {
                let function = eval_expr(*function, env);
                match function {
                    Value::Function(param, body, fn_env) =>
                        match VarEnv::associate(
                            param,
                            eval_expr(*arg, env),
                            &fn_env.unwrap()
                        ) {
                            Ok(fn_env) => eval_expr(*body, &fn_env),
                            Err(error) => Value::Error(error),
                        },
                    _ => error(&format!(
                            "Can't apply argument to type '{}'",
                            function.type_()
                         ))
                }
            }
        Expr::Match(val, patterns) => {
            let val = eval_expr(*val, env);
            match patterns.into_iter().find_map(
                |(pattern, expr)|
                VarEnv::associate(pattern, val.clone(), env)
                .map(|env| Some((env, expr))).unwrap_or(None)
            ) {
                Some((env, expr)) => eval_expr(expr, &env),
                None => error("Value didn't match any patterns")
            }
        }
    }
}

fn error(message: &str) -> Value {
    Value::Error(message.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::expr::{BinaryOp, UnaryOp};
    use crate::ast::match_::Match;
    
    #[test]
    fn eval_literal() {
        let expected = Value::Int(1);
        let actual = eval_expr(Expr::int(1), &VarEnv::empty());
        assert_eq!(expected, actual);
    }
    
    #[test]
    fn eval_binary() {
        let expected = Value::Int(3);
        let actual = eval_expr(
            Expr::binary(
                Expr::int(1),
                BinaryOp::Plus,
                Expr::int(2)
            ),
            &VarEnv::empty()
        );
        assert_eq!(expected, actual);
    }
    
    #[test]
    fn eval_unary() {
        let expected = Value::Int(-3);
        let actual = eval_expr(
            Expr::unary(
                UnaryOp::Negate,
                Expr::int(3)
            ),
            &VarEnv::empty()
        );
        assert_eq!(expected, actual);
    }
    
    #[test]
    fn eval_if_expr() {
        let expected = Value::Int(1);
        let actual = eval_expr(
            Expr::if_expr(
                Expr::bool(true),
                Expr::int(1),
                Expr::int(2)
            ),
            &VarEnv::empty()
        );
        assert_eq!(expected, actual);
        
        let expected = Value::Int(2);
        let actual = eval_expr(
            Expr::if_expr(
                Expr::bool(false),
                Expr::int(1),
                Expr::int(2)
            ),
            &VarEnv::empty()
        );
        assert_eq!(expected, actual);
    }
    
    basic_test! {
        eval_variable
        eval_expr(
            Expr::variable("a"),
            &VarEnv::associate(
                Match::ident("a"),
                Value::Int(1),
                &VarEnv::empty()
            ).unwrap()
        ) => Value::Int(1);
        eval_expr(
            Expr::variable("b"),
            &VarEnv::associate(
                Match::ident("a"),
                Value::Int(1),
                &VarEnv::empty()
            ).unwrap()
        ) => Value::Error("Variable 'b' is not declared".to_string())
    }
    
    basic_test! {
        eval_let_expr
        eval_expr(
            Expr::let_expr(
                Match::ident("a"),
                Expr::int(1),
                Expr::int(2)
            ),
            &VarEnv::empty()
        ) => Value::Int(2);
        eval_expr(
            Expr::let_expr(
                Match::ident("a"),
                Expr::int(1),
                Expr::variable("a")
            ),
            &VarEnv::empty()
        ) => Value::Int(1)
    }
    
    basic_test! {
        eval_fn_application
        eval_expr(
            Expr::fn_app(
                Expr::fn_expr(
                    Match::ident("a"),
                    Expr::variable("a")
                ),
                Expr::int(1)
            ),
            &VarEnv::empty()
        ) => Value::Int(1)
    }
    
    basic_test! {
        match_expr
        eval_expr(
            Expr::match_(Expr::int(1), vec![
                (Match::int(0), Expr::int(0)),
                (Match::int(1), Expr::int(1)),
                (Match::ident("a"), Expr::int(2))
            ]),
            &VarEnv::empty()
        ) => Value::Int(1);
        eval_expr(
            Expr::match_(Expr::int(3), vec![
                (Match::int(0), Expr::int(0)),
                (Match::int(1), Expr::int(1)),
                (Match::int(2), Expr::int(2))
            ]),
            &VarEnv::empty()
        ) => Value::Error("Value didn't match any patterns".to_string())
    }
    
    basic_test! {
        decl_eval
        {
            let decls = vec![
                Decl::Expression(
                    "add1".to_string(),
                    Expr::fn_expr(
                        Match::ident("a"),
                        Expr::plus(Expr::variable("a"), Expr::int(1))
                    )
                )
            ];
            eval_expr(
                Expr::fn_app(
                    Expr::variable("add1"),
                    Expr::int(1)
                ),
                &env_from_decls(&decls)
            )
        } => Value::Int(2);
        {
            let decls = vec![
                Decl::Expression(
                    "a".to_string(),
                    Expr::int(1)
                ),
                Decl::Expression(
                    "b".to_string(),
                    Expr::plus(Expr::variable("a"), Expr::int(1))
                )
            ];
            eval_expr(
                Expr::variable("b"),
                &env_from_decls(&decls)
            )
        } => Value::Int(2)
    }
    
    basic_test! {
        orderless_decl_eval
        {
            let decls = vec![
                Decl::Expression(
                    "a".to_string(),
                    Expr::plus(Expr::variable("b"), Expr::int(1))
                ),
                Decl::Expression(
                    "b".to_string(),
                    Expr::int(2)
                )
            ];
            eval_expr(
                Expr::variable("a"),
                &env_from_decls(&decls)
            )
        } => Value::Int(3)
    }
}