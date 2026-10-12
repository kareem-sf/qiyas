//! The expressions of dimensional constraints and parameters: `d1 = 2*d2 + 1.5`.
//!
//! Numbers (Arabic-Indic digits too), `+ - * /`, parentheses, unary minus, names of other
//! parameters, `pi`, and the functions `sin`, `cos`, `tan` (degrees, as angular constraints are
//! shown), `sqrt` and `abs`. Our own parser, about as small as the language, so no expression
//! library (and its licence) comes along.

use std::collections::{BTreeMap, BTreeSet};

/// A parsed expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Number(f64),
    Name(String),
    Neg(Box<Expr>),
    Binary(Op, Box<Expr>, Box<Expr>),
    Call(Func, Box<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Func {
    Sin,
    Cos,
    Tan,
    Sqrt,
    Abs,
}

impl Func {
    fn named(name: &str) -> Option<Func> {
        Some(match name.to_ascii_lowercase().as_str() {
            "sin" => Func::Sin,
            "cos" => Func::Cos,
            "tan" => Func::Tan,
            "sqrt" => Func::Sqrt,
            "abs" => Func::Abs,
            _ => return None,
        })
    }

    fn apply(self, x: f64) -> f64 {
        match self {
            Func::Sin => x.to_radians().sin(),
            Func::Cos => x.to_radians().cos(),
            Func::Tan => x.to_radians().tan(),
            Func::Sqrt => x.sqrt(),
            Func::Abs => x.abs(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ExprError {
    /// The text is no expression: what is wrong with it.
    #[error("{0}")]
    Syntax(String),
    /// The expression uses a parameter there is none of.
    #[error("There is no parameter named {0}")]
    Unknown(String),
    /// These parameters depend on themselves, in this order.
    #[error("These parameters depend on each other in a circle: {}", .0.join(" > "))]
    Circular(Vec<String>),
    /// The value is no number (a division by 0, the root of a negative number).
    #[error("{0} is not a number (a division by 0?)")]
    NotANumber(String),
    /// It uses this parameter, which can't be worked out itself.
    #[error("It uses {0}, which can't be worked out")]
    Uses(String),
}

/// True for the names parameters may have: a letter or `_`, then letters, digits or `_`; never
/// a function's name or `pi`.
pub fn is_name(text: &str) -> bool {
    let mut chars = text.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        && Func::named(text).is_none()
        && !text.eq_ignore_ascii_case("pi")
}

/// An expression as it is stored: trimmed, with Arabic-Indic and Persian digits and the Arabic
/// decimal separator in ASCII, so a drawing saved in an older DWG code page keeps it.
pub fn normalize(text: &str) -> String {
    ascii_digits(text.trim())
}

/// Arabic-Indic and Persian digits and the Arabic decimal separator as ASCII.
fn ascii_digits(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '\u{0660}'..='\u{0669}' => char::from(b'0' + (c as u32 - 0x0660) as u8),
            '\u{06F0}'..='\u{06F9}' => char::from(b'0' + (c as u32 - 0x06F0) as u8),
            '\u{066B}' => '.',
            _ => c,
        })
        .collect()
}

struct Parser {
    chars: Vec<char>,
    at: usize,
}

impl Parser {
    fn skip(&mut self) {
        while self.chars.get(self.at).is_some_and(|c| c.is_whitespace()) {
            self.at += 1;
        }
    }

    fn peek(&mut self) -> Option<char> {
        self.skip();
        self.chars.get(self.at).copied()
    }

    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn expr(&mut self) -> Result<Expr, ExprError> {
        let mut left = self.term()?;
        loop {
            let op = match self.peek() {
                Some('+') => Op::Add,
                Some('-') => Op::Sub,
                _ => return Ok(left),
            };
            self.at += 1;
            left = Expr::Binary(op, Box::new(left), Box::new(self.term()?));
        }
    }

    fn term(&mut self) -> Result<Expr, ExprError> {
        let mut left = self.unary()?;
        loop {
            let op = match self.peek() {
                Some('*') => Op::Mul,
                Some('/') => Op::Div,
                _ => return Ok(left),
            };
            self.at += 1;
            left = Expr::Binary(op, Box::new(left), Box::new(self.unary()?));
        }
    }

    fn unary(&mut self) -> Result<Expr, ExprError> {
        if self.eat('-') {
            return Ok(Expr::Neg(Box::new(self.unary()?)));
        }
        if self.eat('+') {
            return self.unary();
        }
        self.primary()
    }

    fn primary(&mut self) -> Result<Expr, ExprError> {
        match self.peek() {
            Some('(') => {
                self.at += 1;
                let inner = self.expr()?;
                if !self.eat(')') {
                    return Err(ExprError::Syntax("A bracket is not closed".into()));
                }
                Ok(inner)
            }
            Some(c) if c.is_ascii_digit() || c == '.' => self.number(),
            Some(c) if c.is_ascii_alphabetic() || c == '_' => {
                let start = self.at;
                while self
                    .chars
                    .get(self.at)
                    .is_some_and(|c| c.is_ascii_alphanumeric() || *c == '_')
                {
                    self.at += 1;
                }
                let name: String = self.chars[start..self.at].iter().collect();
                if self.peek() == Some('(') {
                    let Some(f) = Func::named(&name) else {
                        return Err(ExprError::Syntax(format!("There is no function {name}")));
                    };
                    self.at += 1;
                    let arg = self.expr()?;
                    if !self.eat(')') {
                        return Err(ExprError::Syntax("A bracket is not closed".into()));
                    }
                    return Ok(Expr::Call(f, Box::new(arg)));
                }
                if name.eq_ignore_ascii_case("pi") {
                    return Ok(Expr::Number(std::f64::consts::PI));
                }
                Ok(Expr::Name(name))
            }
            Some(c) => Err(ExprError::Syntax(format!("Unexpected \"{c}\""))),
            None => Err(ExprError::Syntax("The expression ends too soon".into())),
        }
    }

    fn number(&mut self) -> Result<Expr, ExprError> {
        let start = self.at;
        let digits = |p: &mut Parser| {
            while p.chars.get(p.at).is_some_and(|c| c.is_ascii_digit()) {
                p.at += 1;
            }
        };
        digits(self);
        if self.chars.get(self.at) == Some(&'.') {
            self.at += 1;
            digits(self);
        }
        if matches!(self.chars.get(self.at), Some('e' | 'E')) {
            let back = self.at;
            self.at += 1;
            if matches!(self.chars.get(self.at), Some('+' | '-')) {
                self.at += 1;
            }
            let exp = self.at;
            digits(self);
            if self.at == exp {
                self.at = back;
            }
        }
        let text: String = self.chars[start..self.at].iter().collect();
        text.parse()
            .map(Expr::Number)
            .map_err(|_| ExprError::Syntax(format!("\"{text}\" is no number")))
    }
}

impl Expr {
    /// Parses `text`, all of it.
    pub fn parse(text: &str) -> Result<Expr, ExprError> {
        let mut p = Parser {
            chars: ascii_digits(text).chars().collect(),
            at: 0,
        };
        let e = p.expr()?;
        match p.peek() {
            None => Ok(e),
            Some(c) => Err(ExprError::Syntax(format!("Unexpected \"{c}\""))),
        }
    }

    /// The value, with `value` giving each name's.
    pub fn eval(&self, value: &dyn Fn(&str) -> Option<f64>) -> Result<f64, ExprError> {
        let v = self.value(value)?;
        if v.is_finite() {
            Ok(v)
        } else {
            Err(ExprError::NotANumber(self.to_string()))
        }
    }

    fn value(&self, value: &dyn Fn(&str) -> Option<f64>) -> Result<f64, ExprError> {
        Ok(match self {
            Expr::Number(n) => *n,
            Expr::Name(n) => value(n).ok_or_else(|| ExprError::Unknown(n.clone()))?,
            Expr::Neg(e) => -e.value(value)?,
            Expr::Binary(op, a, b) => {
                let (a, b) = (a.value(value)?, b.value(value)?);
                match op {
                    Op::Add => a + b,
                    Op::Sub => a - b,
                    Op::Mul => a * b,
                    Op::Div => a / b,
                }
            }
            Expr::Call(f, e) => f.apply(e.value(value)?),
        })
    }

    /// The parameter names it uses, each once, in order of first use.
    pub fn names(&self) -> Vec<String> {
        fn walk(e: &Expr, out: &mut Vec<String>) {
            match e {
                Expr::Name(n) => {
                    if !out.contains(n) {
                        out.push(n.clone());
                    }
                }
                Expr::Neg(e) | Expr::Call(_, e) => walk(e, out),
                Expr::Binary(_, a, b) => {
                    walk(a, out);
                    walk(b, out);
                }
                Expr::Number(_) => {}
            }
        }
        let mut out = Vec::new();
        walk(self, &mut out);
        out
    }
}

impl std::fmt::Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Expr::Number(n) => write!(f, "{n}"),
            Expr::Name(n) => write!(f, "{n}"),
            Expr::Neg(e) => write!(f, "-({e})"),
            Expr::Binary(op, a, b) => {
                let sign = match op {
                    Op::Add => '+',
                    Op::Sub => '-',
                    Op::Mul => '*',
                    Op::Div => '/',
                };
                write!(f, "({a} {sign} {b})")
            }
            Expr::Call(func, e) => write!(f, "{}({e})", format!("{func:?}").to_lowercase()),
        }
    }
}

/// Every parameter's value, from each one's expression (`name -> expression`). A parameter may
/// use any other; one that depends on itself, straight or through others, is refused
/// (`Circular`), as is one using a name not in `exprs` (`Unknown`).
pub fn evaluate_all(exprs: &BTreeMap<String, String>) -> Result<BTreeMap<String, f64>, ExprError> {
    let mut parsed = BTreeMap::new();
    for (name, text) in exprs {
        let e = Expr::parse(text).map_err(|e| match e {
            ExprError::Syntax(why) => ExprError::Syntax(format!("{name}: {why}")),
            other => other,
        })?;
        for used in e.names() {
            if !exprs.contains_key(&used) {
                return Err(ExprError::Unknown(used));
            }
        }
        parsed.insert(name.as_str(), e);
    }
    // Depth first, in name order: a name met again while still open closes a circle.
    let mut values: BTreeMap<String, f64> = BTreeMap::new();
    let mut open: Vec<&str> = Vec::new();
    let mut opened: BTreeSet<&str> = BTreeSet::new();
    fn visit<'a>(
        name: &'a str,
        parsed: &'a BTreeMap<&'a str, Expr>,
        values: &mut BTreeMap<String, f64>,
        open: &mut Vec<&'a str>,
        opened: &mut BTreeSet<&'a str>,
    ) -> Result<(), ExprError> {
        if values.contains_key(name) {
            return Ok(());
        }
        if opened.contains(name) {
            let from = open.iter().position(|n| *n == name).unwrap_or(0);
            let mut circle: Vec<String> = open[from..].iter().map(|n| n.to_string()).collect();
            circle.push(name.to_string());
            return Err(ExprError::Circular(circle));
        }
        let (key, e) = parsed.get_key_value(name).expect("checked known");
        opened.insert(*key);
        open.push(*key);
        for used in e.names() {
            let (used, _) = parsed.get_key_value(used.as_str()).expect("checked known");
            visit(used, parsed, values, open, opened)?;
        }
        open.pop();
        let v = e
            .eval(&|n| values.get(n).copied())
            .map_err(|err| match err {
                ExprError::NotANumber(_) => ExprError::NotANumber(name.to_string()),
                other => other,
            })?;
        values.insert(name.to_string(), v);
        Ok(())
    }
    for name in parsed.keys() {
        visit(name, &parsed, &mut values, &mut open, &mut opened)?;
    }
    Ok(values)
}

/// Every parameter's value, or why it has none, each on its own: one that can't be parsed, uses
/// a name not in `exprs`, sits in a circle or is no number has an error, and so does every one
/// using it (`Uses`); the others are worked out as usual.
pub fn evaluate_each(exprs: &BTreeMap<String, String>) -> BTreeMap<String, Result<f64, ExprError>> {
    let parsed: BTreeMap<&str, Result<Expr, ExprError>> = exprs
        .iter()
        .map(|(name, text)| (name.as_str(), Expr::parse(text)))
        .collect();
    let mut out: BTreeMap<String, Result<f64, ExprError>> = BTreeMap::new();
    let mut open: Vec<String> = Vec::new();
    fn visit(
        name: &str,
        parsed: &BTreeMap<&str, Result<Expr, ExprError>>,
        out: &mut BTreeMap<String, Result<f64, ExprError>>,
        open: &mut Vec<String>,
    ) -> Result<f64, ExprError> {
        if let Some(done) = out.get(name) {
            return done.clone();
        }
        if let Some(from) = open.iter().position(|n| n == name) {
            let mut circle = open[from..].to_vec();
            circle.push(name.to_string());
            return Err(ExprError::Circular(circle));
        }
        open.push(name.to_string());
        let result = match &parsed[name] {
            Err(e) => Err(e.clone()),
            Ok(e) => {
                let mut failed = None;
                for used in e.names() {
                    if !parsed.contains_key(used.as_str()) {
                        failed = Some(ExprError::Unknown(used));
                        break;
                    }
                    match visit(&used, parsed, out, open) {
                        Ok(_) => {}
                        Err(ExprError::Circular(c)) => {
                            failed = Some(ExprError::Circular(c));
                            break;
                        }
                        Err(_) => {
                            failed = Some(ExprError::Uses(used));
                            break;
                        }
                    }
                }
                match failed {
                    Some(e) => Err(e),
                    None => e
                        .eval(&|n| out.get(n).and_then(|r| r.as_ref().ok().copied()))
                        .map_err(|_| ExprError::NotANumber(name.to_string())),
                }
            }
        };
        open.pop();
        out.insert(name.to_string(), result.clone());
        result
    }
    for name in parsed.keys() {
        let _ = visit(name, &parsed, &mut out, &mut open);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arabic_digits_and_names() {
        let none = |_: &str| None;
        assert_eq!(Expr::parse("١٢٫٥").unwrap().eval(&none).unwrap(), 12.5);
        assert!(is_name("d1") && is_name("_w") && is_name("width2"));
        assert!(!is_name("1d") && !is_name("sin") && !is_name("PI") && !is_name("a-b"));
    }
}
