use log::{debug, error};

pub struct PatternPosition {
    pub index: usize,
    pub length: usize,
}

pub fn find_last_separator(input: &String) -> Option<PatternPosition> {
    let pattern_res = regex::Regex::new(r#"[\s+.\-/*\(\)=]"#);
    // let pattern_res = regex::Regex::new(r#"\W*\w*$"#);
    if let Err(e) = &pattern_res {
        error!("Could not compile pattern {}", e);
    } else if let Ok(pattern) = &pattern_res {
        if let Some(ma) = pattern.find_iter(input.as_str()).last() {
            debug!("Last match in input string found {:?}", ma);
            return Some(PatternPosition {
                index: ma.start(),
                length: ma.end() - ma.start(),
            });
        }
    }
    return None;
}
