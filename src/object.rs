use std::{cell::RefCell, fmt, rc::Rc};

use thiserror::Error;

use crate::{
    ast::{ParserError, Statement},
    environment::Environment,
    token::TokenKind,
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Object {
    IntegerValue(i32),
    BooleanValue(bool),
    StringValue(String),
    ReturnValue(Box<Object>),
    FunctionValue(Closure),
    BuiltinValue(BuiltinFunction),
    UnitValue,
}

impl fmt::Display for Object {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Object::IntegerValue(value) => write!(f, "{value}"),
            Object::BooleanValue(value) => write!(f, "{value}"),
            Object::StringValue(value) => write!(f, "\"{value}\""),
            Object::FunctionValue(value) => write!(f, "{value}"),
            Object::ReturnValue(value) => write!(f, "return {value}"),
            Object::BuiltinValue(value) => write!(f, "built-in function {value}"),
            Object::UnitValue => write!(f, "()"),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Closure {
    pub parameters: Vec<String>,
    pub body: Statement,
    pub env: Rc<RefCell<Environment>>,
}

impl fmt::Display for Closure {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "fn({}) {}", self.parameters.join(", "), self.body)
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum BuiltinFunction {
    Len,
    Push,
}

impl BuiltinFunction {
    /// Matches built-in functions.
    pub fn lookup_function(identifier: &str) -> Result<Object, EvalError> {
        match identifier {
            "len" => Ok(Object::BuiltinValue(BuiltinFunction::Len)),
            "push" => Ok(Object::BuiltinValue(BuiltinFunction::Push)),
            _ => Err(EvalError::IdentifierNotFound(identifier.to_owned())),
        }
    }
}

impl fmt::Display for BuiltinFunction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            BuiltinFunction::Len => write!(f, "let"),
            BuiltinFunction::Push => write!(f, "push"),
        }
    }
}

#[derive(Error, Debug)]
pub enum EvalError {
    #[error("Identifier not found: {0}")]
    IdentifierNotFound(String),

    #[error("Type mismatch: {0}")]
    TypeMismatch(String),

    #[error("Modulo of zero isn't allowed")]
    ModuloByZero,

    #[error("Division by zero isn't allowed")]
    DivisionByZero,

    #[error("Function not found: {0}")]
    FunctionNotFound(String),

    #[error("Function call with the wrong number of arguments. Expected {0}, got {1}")]
    FunctionCallWrongArity(u8, u8),

    #[error("Return statement used outside an expression")]
    ReturnOutsideExpression,

    #[error("Unsupported operator: {0}")]
    UnsupportedOperator(TokenKind),

    #[error("Parsing error: {0}")]
    ParsingError(#[from] ParserError),

    #[error("Unknown evaluation error")]
    Unknown,
}
