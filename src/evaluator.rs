use std::{cell::RefCell, rc::Rc};

use crate::{
    ast::{Expression, Statement},
    environment::Environment,
    object::{Closure, EvalError, Object},
    parser::Parser,
    token::TokenKind,
};

#[derive(Debug)]
pub struct Evaluator<'a> {
    parser: Parser<'a>,
    env: Rc<RefCell<Environment>>,
    returned_value: Option<Object>,
}

impl<'a> Evaluator<'a> {
    pub fn new(input: &'a str) -> Self {
        let parser = Parser::new(input);
        let env = Rc::new(RefCell::new(Environment::default()));

        Evaluator {
            parser,
            env,
            returned_value: None,
        }
    }

    pub fn eval_program(&mut self) -> Result<Vec<Object>, EvalError> {
        let program = self.parser.parse_program()?;
        let mut objects: Vec<Object> = vec![];

        for statement in program.0 {
            let obj = self.eval_statement(statement)?;
            objects.push(obj);
        }

        Ok(objects)
    }

    fn eval_statement(&mut self, statement: Statement) -> Result<Object, EvalError> {
        match statement {
            Statement::VarStatement {
                kind: _,
                name,
                value,
            } => {
                let obj = self.eval_expression(value)?;
                self.env.borrow_mut().set(name, obj);
                Ok(Object::UnitValue)
            }
            Statement::ReturnStatement(expr) => {
                let obj = self.eval_expression(expr)?;
                self.returned_value = Some(obj.clone());
                Ok(Object::ReturnValue(Box::new(obj)))
            }
            Statement::ExpressionStatement(expr) => Ok(self.eval_expression(expr)?),
            Statement::BlockStatement(statements) => {
                let inner_env = self.create_enclosed_env();
                let outer_env = std::mem::replace(&mut self.env, inner_env);

                // save last evaluated object
                let mut obj = Object::UnitValue;
                for statement in statements {
                    if let Some(returned_value) = &self.returned_value {
                        // obj = returned_value.clone();
                        self.returned_value = None;
                        break;
                    }

                    obj = self.eval_statement(statement)?;

                    // if the current object is a `return` value, stop evaluating this block
                    if let Object::ReturnValue(_) = obj {
                        // obj = *inner_obj.clone();
                        break;
                    }
                }

                // go back to the outer environment
                self.env = outer_env;

                // return the last evaluated object
                Ok(obj)
            }
        }
    }

    fn eval_expression(&mut self, expr: Expression) -> Result<Object, EvalError> {
        let obj = match expr {
            Expression::IntegerLiteral(lit) => Object::IntegerValue(lit),
            Expression::BooleanLiteral(lit) => Object::BooleanValue(lit),
            Expression::Identifier(name) => self.env.borrow().get(&name)?,
            Expression::BinaryExpression {
                left,
                operator,
                right,
            } => self.eval_binary_expression(*left, operator, *right)?,
            Expression::UnaryExpression { operator, value } => {
                self.eval_unary_expression(operator, *value)?
            }
            Expression::GroupedExpression(expr) => self.eval_expression(*expr)?,
            Expression::CallExpression { path, arguments } => {
                self.eval_call_expression(path, arguments)?
            }
            Expression::IfExpression {
                condition,
                consequence,
                alternative,
            } => self.eval_if_expression(*condition, *consequence, alternative)?,
            Expression::FunctionExpression { parameters, body } => {
                self.eval_function_expression(parameters, *body)?
            }
        };

        Ok(obj)
    }

    fn eval_binary_expression(
        &mut self,
        left: Expression,
        operator: TokenKind,
        right: Expression,
    ) -> Result<Object, EvalError> {
        let left_obj = self.eval_expression(left)?;
        let right_obj = self.eval_expression(right)?;

        let obj = match (left_obj, right_obj) {
            (Object::IntegerValue(lhs), Object::IntegerValue(rhs)) => match operator {
                TokenKind::Plus => Object::IntegerValue(lhs + rhs),
                TokenKind::Minus => Object::IntegerValue(lhs - rhs),
                TokenKind::Asterisk => Object::IntegerValue(lhs * rhs),
                TokenKind::Equal => Object::BooleanValue(lhs == rhs),
                TokenKind::NotEqual => Object::BooleanValue(lhs != rhs),
                TokenKind::LessThan => Object::BooleanValue(lhs < rhs),
                TokenKind::GreaterThan => Object::BooleanValue(lhs > rhs),
                TokenKind::LessThanEqual => Object::BooleanValue(lhs <= rhs),
                TokenKind::GreaterThanEqual => Object::BooleanValue(lhs >= rhs),
                TokenKind::Percentage => {
                    if rhs == 0 {
                        return Err(EvalError::ModuloByZero);
                    } else {
                        Object::IntegerValue(lhs % rhs)
                    }
                }
                TokenKind::Slash => {
                    if rhs == 0 {
                        return Err(EvalError::DivisionByZero);
                    } else {
                        Object::IntegerValue(lhs / rhs)
                    }
                }
                _ => return Err(EvalError::UnsupportedOperator(operator)),
            },

            (Object::BooleanValue(lhs), Object::BooleanValue(rhs)) => match operator {
                TokenKind::Equal => Object::BooleanValue(lhs == rhs),
                TokenKind::NotEqual => Object::BooleanValue(lhs != rhs),
                _ => return Err(EvalError::UnsupportedOperator(operator)),
            },

            (lhs, rhs) => {
                return Err(EvalError::TypeMismatch(format!(
                    "Cannot perform operation '{operator}' between '{lhs}' and '{rhs}'",
                )));
            }
        };

        Ok(obj)
    }

    fn eval_unary_expression(
        &mut self,
        operator: TokenKind,
        value: Expression,
    ) -> Result<Object, EvalError> {
        let obj = match operator {
            TokenKind::Bang => match self.eval_expression(value)? {
                Object::IntegerValue(lit) => Object::IntegerValue(!lit),
                Object::BooleanValue(lit) => Object::BooleanValue(!lit),
                _ => return Err(EvalError::UnsupportedOperator(operator)),
            },

            TokenKind::Minus => match self.eval_expression(value)? {
                Object::IntegerValue(lit) => Object::IntegerValue(-lit),
                _ => return Err(EvalError::UnsupportedOperator(operator)),
            },

            _ => return Err(EvalError::UnsupportedOperator(operator)),
        };

        Ok(obj)
    }

    fn eval_if_expression(
        &mut self,
        condition: Expression,
        consequence: Statement,
        alternative: Option<Box<Statement>>,
    ) -> Result<Object, EvalError> {
        let obj = match self.eval_expression(condition)? {
            Object::BooleanValue(lit) => {
                if lit {
                    self.eval_statement(consequence)?
                } else if let Some(alt) = alternative {
                    self.eval_statement(*alt)?
                } else {
                    Object::UnitValue
                }
            }
            _ => {
                return Err(EvalError::TypeMismatch(
                    "`if` condition must be a boolean".to_owned(),
                ))
            }
        };

        Ok(obj)
    }

    fn eval_function_expression(
        &mut self,
        parameters: Vec<String>,
        body: Statement,
    ) -> Result<Object, EvalError> {
        let closure = Closure {
            parameters,
            body,
            env: self.create_enclosed_env(),
        };

        Ok(Object::FunctionValue(closure))
    }

    fn eval_call_expression(
        &mut self,
        path: String,
        arguments: Vec<Expression>,
    ) -> Result<Object, EvalError> {
        let function = self.env.borrow().get(&path)?;

        let obj = match function {
            Object::FunctionValue(Closure {
                parameters,
                body,
                env,
            }) => {
                if parameters.len() != arguments.len() {
                    return Err(EvalError::FunctionCallWrongArity(
                        parameters.len() as u8,
                        arguments.len() as u8,
                    ));
                }

                // evaluate arguments in the current scope
                let arguments = arguments
                    .into_iter()
                    .map(|arg| self.eval_expression(arg))
                    .collect::<Result<Vec<Object>, EvalError>>()?;

                // switch to the closure environment
                let outer_env = std::mem::replace(&mut self.env, env);

                // add bindings in the closure environment
                for (param, arg) in parameters.into_iter().zip(arguments.into_iter()) {
                    self.env.borrow_mut().set(param, arg);
                }

                // evaluate the closure body
                let body_obj = self.eval_statement(body)?;
                // go back to the old environment
                self.env = outer_env;

                body_obj
            }

            _ => {
                return Err(EvalError::FunctionNotFound(
                    "Check if this identifier is a declared function".to_owned(),
                ));
            }
        };

        Ok(obj)
    }

    /// Creates a new environment linked to the outer environment
    fn create_enclosed_env(&mut self) -> Rc<RefCell<Environment>> {
        let inner_env = Environment {
            outer: Some(self.env.clone()),
            ..Default::default()
        };
        Rc::new(RefCell::new(inner_env))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eval_integer_literal() {
        let input = "5";
        let mut evaluator = Evaluator::new(input);
        let result = &evaluator.eval_program().unwrap()[0];
        assert_eq!(result, &Object::IntegerValue(5));
    }

    #[test]
    fn eval_boolean_literal() {
        let input = "true";
        let mut evaluator = Evaluator::new(input);
        let result = &evaluator.eval_program().unwrap()[0];
        assert_eq!(result, &Object::BooleanValue(true));
    }

    #[test]
    fn eval_boolean_expressions() {
        let tests = vec![
            ("true", true),
            ("false", false),
            ("1 < 2", true),
            ("1 > 2", false),
            ("1 < 1", false),
            ("1 > 1", false),
            ("1 == 1", true),
            ("1 != 1", false),
            ("1 == 2", false),
            ("1 != 2", true),
            ("true == true", true),
            ("false == false", true),
            ("true == false", false),
            ("true != false", true),
            ("false != true", true),
            ("(1 < 2) == true", true),
            ("(1 < 2) == false", false),
            ("(1 > 2) == true", false),
            ("(1 > 2) == false", true),
        ];

        for (input, expected) in tests {
            let mut evaluator = Evaluator::new(input);
            let result = &evaluator.eval_program().unwrap()[0];

            let expected_obj = match expected {
                true => &Object::BooleanValue(true),
                false => &Object::BooleanValue(false),
            };

            assert_eq!(result, expected_obj);
        }
    }

    #[test]
    fn eval_binary_expressions() {
        let tests = vec![
            ("2 + 3", &Object::IntegerValue(5)),
            ("4 - 1", &Object::IntegerValue(3)),
            ("5 * 6", &Object::IntegerValue(30)),
            ("10 / 2", &Object::IntegerValue(5)),
            ("7 == 7", &Object::BooleanValue(true)),
            ("8 != 9", &Object::BooleanValue(true)),
            ("true == true", &Object::BooleanValue(true)),
            ("false != true", &Object::BooleanValue(true)),
        ];

        for (input, expected) in tests {
            let mut evaluator = Evaluator::new(input);
            let result = &evaluator.eval_program().unwrap()[0];
            assert_eq!(result, expected);
        }
    }

    #[test]
    fn eval_unary_expressions() {
        let tests = vec![
            ("-2", &Object::IntegerValue(-2)),
            ("!true", &Object::BooleanValue(false)),
            ("!false", &Object::BooleanValue(true)),
            ("!5", &Object::IntegerValue(-6)),
            ("!!5", &Object::IntegerValue(5)),
            ("!0", &Object::IntegerValue(-1)),
            ("!!true", &Object::BooleanValue(true)),
            ("!!false", &Object::BooleanValue(false)),
        ];

        for (input, expected) in tests {
            let mut evaluator = Evaluator::new(input);
            let result = &evaluator.eval_program().unwrap()[0];
            assert_eq!(result, expected);
        }
    }

    #[test]
    fn eval_if_expression() {
        let tests = vec![
            ("if true { 10 }", &Object::IntegerValue(10)),
            ("if false { 10 }", &Object::UnitValue),
            ("if 1 < 2 { 10 }", &Object::IntegerValue(10)),
            ("if 1 > 2 { 10 }", &Object::UnitValue),
            ("if 1 > 2 { 10 } else { 20 }", &Object::IntegerValue(20)),
            ("if 1 < 2 { 10 } else { 20 }", &Object::IntegerValue(10)),
        ];

        for (input, expected) in tests {
            let mut evaluator = Evaluator::new(input);
            let result = &evaluator.eval_program().unwrap()[0];
            assert_eq!(result, expected);
        }
    }

    #[test]
    fn eval_function_expression() {
        let input = r#"
            let foo = fn(x) {
                let double = fn(y) { y * 2; };
                return double(x);
            };

            let bar = foo(3);
            bar;
        "#;
        let mut evaluator = Evaluator::new(input);
        let result = &evaluator.eval_program().unwrap()[2];
        assert_eq!(result, &Object::IntegerValue(6));
    }

    #[test]
    fn eval_function_expressions() {
        let tests = vec![
            ("let identity = fn(x) { x; }; identity(5);", 5),
            ("let identity = fn(x) { return x; }; identity(5);", 5),
            ("let double = fn(x) { x * 2; }; double(5);", 10),
            ("let add = fn(x, y) { x + y; }; add(5, 5);", 10),
            ("let add = fn(x, y) { x + y; }; add(5 + 5, add(5, 5));", 20),
            // ("fn(x) { x; }(5)", 5),
        ];

        for (input, expected) in tests {
            let mut evaluator = Evaluator::new(input);
            let result = &evaluator.eval_program().unwrap()[1];
            let expected_obj = &Object::IntegerValue(expected);
            assert_eq!(result, expected_obj);
        }
    }

    #[test]
    fn eval_block_statement() {
        let input = r#"
            let a = 2;

            {
                let b = 3;
                b;
            }

            a;
        "#;
        let mut evaluator = Evaluator::new(input);
        let result = &evaluator.eval_program().unwrap()[2];
        assert_eq!(result, &Object::IntegerValue(2));
    }

    #[test]
    fn eval_static_scope() {
        let input = r#"
            let i = 5;
            let foo = fn(i) {
                i;
            };

            foo(10);
            i;
        "#;
        let mut evaluator = Evaluator::new(input);
        let result = &evaluator.eval_program().unwrap();
        assert_eq!(&result[2], &Object::IntegerValue(10));
        assert_eq!(&result[3], &Object::IntegerValue(5));
    }

    #[test]
    fn eval_closure() {
        let input = r#"
            let newAdder = fn(x) {
                fn(y) { x + y };
            };

            let addTwo = newAdder(2);
            addTwo(2);
        "#;
        let mut evaluator = Evaluator::new(input);
        let result = &evaluator.eval_program().unwrap();
        assert_eq!(&result[2], &Object::IntegerValue(4));
    }

    // #[test]
    // fn eval_nested_returns() {
    //     let input = r#"
    //         let bar = fn() { return 2; };
    //         let baz = if true { 2; };

    //         let foo = if bar() + 1 == 3 {
    //             if true {
    //                 {
    //                     return fn(x) { x; };
    //                 }
    //             }

    //             return 1;
    //         };

    //         let id = foo(3);
    //         id;
    //     "#;
    //     let mut evaluator = Evaluator::new(input);
    //     evaluator.eval_program().unwrap();
    // }
}
