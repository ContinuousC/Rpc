/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use std::fmt::Write;

pub fn pascal_case(val: &str) -> String {
    let mut res = String::new();
    for word in val.split('_') {
        if word.is_empty() {
            continue;
        }
        write!(res, "{}{}", word[0..1].to_uppercase(), &word[1..]).unwrap();
    }
    res
}

pub fn snake_case(val: &str) -> String {
    let mut res = String::new();
    for c in val.chars() {
        match c.is_uppercase() {
            true => {
                if !res.is_empty() {
                    res.push('_');
                }
                res.extend(c.to_lowercase());
            }
            false => {
                res.push(c);
            }
        }
    }
    res
}
