use std::sync::Arc;

use hemtt_common::reporting::{Code, Token};

use crate::Error;

#[allow(unused)]
/// The EOI was reached while reading an `#if` [`IfState`]
///
/// ```cpp
/// #if 1
/// #else
/// EOI
/// ```
pub struct EoiIfState {
    /// The [`Token`] of the last `#if`
    token: Box<Token>,
}

impl Code for EoiIfState {
    fn ident(&self) -> &'static str {
        "PE18"
    }

    fn token(&self) -> Option<&Token> {
        Some(&self.token)
    }

    fn message(&self) -> String {
        "end of input reached while reading an #if directive".to_string()
    }

    fn label_message(&self) -> String {
        "last #if directive".to_string()
    }
}

impl EoiIfState {
    pub fn new(token: Box<Token>) -> Self {
        Self { token }
    }

    pub fn code(token: Token) -> Error {
        Error::Code(Arc::new(Self::new(Box::new(token))))
    }
}
