pub struct PatternPosition {
    pub index: usize,
    pub length: usize,
}

macro_rules! find_separators {
    ($input : expr, $separators: expr) => {
        $separators
            .find_iter($input.as_str())
            .map(|m| PatternPosition {
                index: m.start(),
                length: m.end() - m.start(),
            })
    };
}

pub fn find_first_separator(input: &String) -> Option<PatternPosition> {
    let pattern = regex::Regex::new(r#"\W+"#).unwrap();
    let val = find_separators!(input, pattern).next();
    val
}

pub fn find_last_separator(input: &String) -> Option<PatternPosition> {
    let pattern = regex::Regex::new(r#"\W+"#).unwrap();
    let val = find_separators!(input, pattern).last();
    val
}
