use serde::{Serialize, Deserialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LogoValue {
    Number(f64),
    Word(String),
    List(Vec<LogoValue>),
}

impl LogoValue {
    pub fn as_number(&self) -> Result<f64, String> {
        match self {
            LogoValue::Number(n) => Ok(*n),
            LogoValue::Word(s) => s
                .parse::<f64>()
                .map_err(|_| format!("Expected a number, got {}", s)),
            _ => Err(format!("Expected a number, got {}", self)),
        }
    }

    pub fn as_string(&self) -> String {
        match self {
            LogoValue::Number(n) => {
                if *n == n.floor() && n.is_finite() {
                    format!("{}", *n as i64)
                } else {
                    format!("{}", n)
                }
            }
            LogoValue::Word(s) => s.clone(),
            LogoValue::List(items) => items
                .iter()
                .map(|v| v.as_string())
                .collect::<Vec<_>>()
                .join(" "),
        }
    }

    pub fn is_truthy(&self) -> Result<bool, String> {
        match self {
            LogoValue::Word(s) if s == "true" => Ok(true),
            LogoValue::Word(s) if s == "false" => Ok(false),
            _ => Err(format!("Expected true or false, got {}", self)),
        }
    }

    pub fn logo_equal(&self, other: &LogoValue) -> bool {
        match (self, other) {
            (LogoValue::Number(a), LogoValue::Number(b)) => a == b,
            _ => self.as_string().to_lowercase() == other.as_string().to_lowercase(),
        }
    }

    pub fn show(&self) -> String {
        match self {
            LogoValue::List(items) => {
                let inner = items.iter().map(|v| v.show()).collect::<Vec<_>>().join(" ");
                format!("[{}]", inner)
            }
            _ => self.as_string(),
        }
    }
}

impl fmt::Display for LogoValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_string())
    }
}
