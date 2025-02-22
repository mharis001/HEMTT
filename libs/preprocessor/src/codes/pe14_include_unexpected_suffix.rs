use std::sync::Arc;

use hemtt_common::reporting::{Code, Token};

use crate::Error;

#[allow(unused)]
/// Unexpected token
pub struct IncludeUnexpectedSuffix {
    /// The [`Token`] that was found
    token: Box<Token>,
}

impl Code for IncludeUnexpectedSuffix {
    fn ident(&self) -> &'static str {
        "PE14"
    }

    fn token(&self) -> Option<&Token> {
        Some(&self.token)
    }

    fn message(&self) -> String {
        "unexpected tokens after include".to_string()
    }

    fn help(&self) -> Option<String> {
        Some("expected a newline after include".to_string())
    }
}

impl IncludeUnexpectedSuffix {
    pub fn new(token: Box<Token>) -> Self {
        Self { token }
    }

    pub fn code(token: Token) -> Error {
        Error::Code(Arc::new(Self::new(Box::new(token))))
    }
}
