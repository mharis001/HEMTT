#![allow(clippy::module_name_repetitions)]

use rhai::Position;

pub mod bhe1_script_not_found;
pub mod bhe2_script_fatal;
pub mod bhe3_parse_error;
pub mod bhe4_runtime_error;

fn get_offset(content: &str, location: Position) -> usize {
    let mut offset = 0;
    for (i, line) in content.lines().enumerate() {
        if i == location.line().unwrap() {
            offset += location.position().unwrap();
            break;
        }
        offset += line.len() + 1;
    }
    offset
}
