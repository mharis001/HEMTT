use std::sync::Arc;

use hemtt_common::reporting::{Code, Diagnostic};

pub struct WorkshopModNotFound {
    id: String,
}

impl Code for WorkshopModNotFound {
    fn ident(&self) -> &'static str {
        "BCLE2"
    }

    fn message(&self) -> String {
        format!("Arma 3 workshop mod `{}` not found.", self.id)
    }

    fn help(&self) -> Option<String> {
        Some("HEMTT does not subscribe to mods, you must subscribe in Steam and allow it to download.".to_owned())
    }

    fn diagnostic(&self) -> Option<Diagnostic> {
        Some(Diagnostic::simple(self))
    }
}

impl WorkshopModNotFound {
    pub fn code(id: String) -> Arc<dyn Code> {
        Arc::new(Self { id })
    }
}
