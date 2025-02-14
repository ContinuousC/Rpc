/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use std::collections::HashMap;
use std::fmt::Write;

use nom::{
    branch::alt,
    bytes::complete::{tag, take_while, take_while1},
    combinator::{eof, recognize},
    sequence::{delimited, terminated},
    Finish, IResult,
};

/// Simple template used for auto-generating API definitions
/// for other programming languages. If this were to be used
/// for production code, it should be updated to use errors
/// instead of panics!
pub struct Template<'a>(Vec<Elem<'a>>);

pub enum Elem<'a> {
    Text(&'a str),
    Variable(&'a str),
    Indented(&'a str, &'a str, &'a str),
}

impl<'a> Template<'a> {
    pub fn parse(input: &'a str) -> Self {
        match Finish::finish(parse_template(input)) {
            Ok((_, tmpl)) => tmpl,
            Err(e) => panic!("Failed to parse template: {}", e),
        }
    }

    pub fn fill(&self, vars: HashMap<&str, &str>) -> String {
        let mut output = String::new();
        for elem in &self.0 {
            match elem {
                Elem::Text(text) => write!(output, "{}", text).unwrap(),
                Elem::Variable(var) => match vars.get(var) {
                    Some(val) => write!(output, "{}", val).unwrap(),
                    None => panic!("Missing template variable '{}'", var),
                },
                Elem::Indented(start, var, end) => match vars.get(var) {
                    Some(val) => {
                        for line in val.lines() {
                            write!(output, "{}{}{}", start, line, end).unwrap();
                        }
                    }
                    None => panic!("Missing template variable '{}'", var),
                },
            }
        }
        output
    }
}

fn parse_template(mut input: &str) -> IResult<&str, Template> {
    let mut elems = Vec::new();
    let mut new_line = true;
    let mut i = 0;

    while !input[i..].is_empty() {
        if new_line {
            if let Ok((next, elem)) = indented(&input[i..]) {
                if i > 0 {
                    elems.push(Elem::Text(&input[..i]));
                    i = 0;
                }
                elems.push(elem);
                input = next;
                continue;
            }
        }

        if let Ok((next, var)) = variable(&input[i..]) {
            if i > 0 {
                elems.push(Elem::Text(&input[..i]));
                i = 0;
            }
            elems.push(Elem::Variable(var));
            input = next;
            continue;
        }

        new_line = input[i..].starts_with('\n');
        i += 1;
    }

    if i > 0 {
        elems.push(Elem::Text(&input[..i]));
        input = &input[i..];
    }

    Ok((input, Template(elems)))
}

fn indented(input: &str) -> IResult<&str, Elem> {
    let (input, indent) = take_while(|c| c == ' ' || c == '\t')(input)?;
    let (input, var_name) = variable(input)?;
    let (input, end) = recognize(terminated(
        take_while(|c| c == ' ' || c == '\t'),
        alt((tag("\n"), eof)),
    ))(input)?;
    Ok((input, Elem::Indented(indent, var_name, end)))
}

fn variable(input: &str) -> IResult<&str, &str> {
    let (input, var_name) = delimited(
        tag("{{"),
        take_while1(|c: char| c.is_alphanumeric()),
        tag("}}"),
    )(input)?;
    Ok((input, var_name))
}
