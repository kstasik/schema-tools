use std::fmt;

use regex::Regex;
use serde::Serialize;

use crate::error::Error;

#[derive(Clone, Debug)]
pub enum SchemaNamingStrategy {
    Default,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct SchemaScope {
    scope: Vec<SchemaScopeType>,
    naming_strategy: SchemaNamingStrategy,
    spaces: Vec<Space>,
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
pub enum Space {
    Tag(String),
    Operation(String),
    Id(String),
    Parameter,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SchemaScopeType {
    // real parts of schema pointer
    Index(usize),
    Property(String),
    Entity(String),
    Form(String),
    Definition(String),
    Reference(String),
    Any(String),

    // name builder
    Glue(String),
}

#[derive(Debug, Clone)]
pub struct BasicNamer {
    parts: Vec<SchemaScopeType>,
}

impl BasicNamer {
    pub fn simple(&self) -> Result<String, Error> {
        Ok(self.build(self.parts()?))
    }

    pub fn build(&self, parts: Vec<String>) -> String {
        let result = parts
            .iter()
            .map(|s| s[..1].to_ascii_uppercase() + &s[1..])
            .collect::<Vec<_>>()
            .join("");

        result
    }

    pub fn convert(&self, original: &str) -> String {
        self.build(self.split(original))
    }

    pub fn decorate(&self, parts: Vec<String>) -> String {
        let current = self.parts().unwrap();
        let parts = [&current[..], &parts[..]].concat();

        self.build(parts)
    }

    fn parts(&self) -> Result<Vec<String>, Error> {
        if self.parts.is_empty() {
            return Err(Error::NotImplemented);
        }

        let form = if self.parts.len() < 2 {
            None
        } else if let Some(SchemaScopeType::Form(form)) = self.parts.get(self.parts.len() - 2) {
            if form == "oneOf" || form == "anyOf" {
                let last = self.parts.last().unwrap();
                match last {
                    SchemaScopeType::Index(i) => Some(format!("Option{}", i + 1)),
                    _ => None,
                }
            } else if form == "allOf" {
                let last = self.parts.last().unwrap();
                match last {
                    SchemaScopeType::Index(i) => Some(format!("Partial{}", i + 1)),
                    _ => None,
                }
            } else {
                None
            }
        } else {
            None
        };

        match self
            .parts
            .iter()
            .filter(|s| {
                !matches!(
                    s,
                    SchemaScopeType::Form(_)
                        | SchemaScopeType::Index(_)
                        | SchemaScopeType::Reference(_)
                )
            })
            .next_back()
            .unwrap()
        {
            SchemaScopeType::Entity(name) => {
                let real_name = form
                    .map(|f| format!("{name}{f}"))
                    .unwrap_or_else(|| name.to_string());
                Ok(self.split(&real_name))
            }
            SchemaScopeType::Property(last) | SchemaScopeType::Definition(last) => {
                let entity = self
                    .parts
                    .clone()
                    .into_iter()
                    .filter_map(|s| match s {
                        SchemaScopeType::Entity(t) => Some(t),
                        _ => None,
                    })
                    .next_back();

                if let Some(name) = entity {
                    let parts = [&self.split(&name)[..], &self.split(last)[..]].concat();

                    Ok(parts)
                } else {
                    Err(Error::NotImplemented)
                }
            }
            _ => {
                let glued: Vec<String> = self
                    .parts
                    .clone()
                    .into_iter()
                    .filter_map(|s| match s {
                        SchemaScopeType::Glue(t) => Some(t),
                        _ => None,
                    })
                    .collect();

                let parts: Vec<String> = glued.iter().flat_map(|s| self.split(s)).collect();
                if !glued.is_empty() {
                    Ok(parts)
                } else {
                    Err(Error::CodegenCannotRetrieveNameError(format!(
                        "parts: {:?}",
                        self.parts
                    )))
                }
            }
        }
    }

    fn split(&self, phrase: &str) -> Vec<String> {
        // todo: refactor
        let re = Regex::new(r"[A-Z_]").unwrap();
        let result = re.replace_all(phrase, " $0");

        let t = result
            .chars()
            .filter(|c| {
                c.is_ascii_alphabetic() || c.is_ascii_alphanumeric() || c.is_ascii_whitespace()
            })
            .collect::<String>();

        let result: Vec<String> = t
            .split(' ')
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>();

        result
    }
}

impl Default for SchemaScope {
    fn default() -> Self {
        Self {
            scope: vec![],
            spaces: vec![],
            naming_strategy: SchemaNamingStrategy::Default,
        }
    }
}

impl SchemaScope {
    pub fn new(naming_strategy: SchemaNamingStrategy) -> Self {
        Self {
            naming_strategy,
            ..Self::default()
        }
    }

    pub fn index(&mut self, index: usize) {
        self.scope.push(SchemaScopeType::Index(index));
    }

    pub fn pop(&mut self) {
        self.scope.pop();
    }

    pub fn reduce(&mut self, n: usize) {
        self.scope.truncate(self.scope.len() - n);
    }

    pub fn len(&self) -> usize {
        self.scope.len()
    }

    pub fn is_empty(&self) -> bool {
        self.scope.is_empty()
    }

    pub fn property(&mut self, property: &str) -> &mut Self {
        self.scope
            .push(SchemaScopeType::Property(property.to_string()));
        self
    }

    pub fn entity(&mut self, title: &str) {
        self.scope.push(SchemaScopeType::Entity(title.to_string()));
    }

    pub fn form(&mut self, form: &str) {
        self.scope.push(SchemaScopeType::Form(form.to_string()));
    }

    pub fn definition(&mut self, form: &str) -> &mut Self {
        self.scope
            .push(SchemaScopeType::Definition(form.to_string()));
        self
    }

    pub fn reference(&mut self, reference: &str) {
        self.scope
            .push(SchemaScopeType::Reference(reference.to_string()));
    }

    pub fn any(&mut self, property: &str) -> &mut Self {
        self.scope.push(SchemaScopeType::Any(property.to_string()));
        self
    }

    pub fn push_str(&mut self, name: &str, what: &str) -> &mut Self {
        match name {
            "property" => self.property(what),
            "definition" => self.definition(what),
            _ => self.any(what),
        }
    }

    pub fn glue(&mut self, property: &str) -> &mut Self {
        self.scope.push(SchemaScopeType::Glue(property.to_string()));
        self
    }

    pub fn add_spaces(&mut self, spaces: &mut Vec<Space>) -> &mut Self {
        self.spaces.append(spaces);
        self
    }

    pub fn add_space(&mut self, space: Space) -> &mut Self {
        self.spaces.push(space);
        self
    }

    pub fn clear_spaces(&mut self) -> &mut Self {
        self.spaces.clear();
        self
    }

    pub fn pop_space(&mut self) -> &mut Self {
        self.spaces.pop();
        self
    }

    pub fn get_spaces(&mut self) -> Vec<Space> {
        self.spaces.clone()
    }

    pub fn namer(&mut self) -> BasicNamer {
        BasicNamer {
            parts: self.scope.clone(),
        }
    }

    pub fn path(&mut self) -> String {
        self.scope
            .iter()
            .rev()
            .find_map(|s| match s {
                SchemaScopeType::Reference(r) => Some(r),
                _ => None,
            })
            // reference exists
            .map(|reference| {
                let mut post = self
                    .scope
                    .rsplit(|sep| *sep == SchemaScopeType::Reference(reference.clone()))
                    .next()
                    .unwrap()
                    .to_vec()
                    .iter()
                    .filter_map(|s| scope_to_string(s.clone()))
                    .collect::<Vec<String>>();

                let mut parts = vec![reference.to_string()];
                parts.append(&mut post);

                parts.join("/")
            })
            .unwrap_or_else(|| format!("{self}"))
    }

    pub fn is_ambiguous(&mut self) -> bool {
        if self.scope.len() < 2 {
            return false;
        }

        if let Some(SchemaScopeType::Form(form)) = self.scope.get(self.scope.len() - 2) {
            form == "oneOf" || form == "anyOf"
        } else {
            false
        }
    }

    pub fn recurse(&self) -> bool {
        if let Some(SchemaScopeType::Reference(reference)) = self.scope.last() {
            self.scope
                .iter()
                .filter(|r| match r {
                    SchemaScopeType::Reference(r) => r == reference,
                    _ => false,
                })
                .count()
                == 2
        } else {
            false
        }
    }
}

impl fmt::Display for SchemaScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            f,
            "/{}",
            self.scope
                .clone()
                .into_iter()
                .filter_map(scope_to_string)
                .collect::<Vec<String>>()
                .join("/")
        )
    }
}

fn scope_to_string(s: SchemaScopeType) -> Option<String> {
    match s {
        SchemaScopeType::Entity(_) => None,
        SchemaScopeType::Glue(_) => None,
        SchemaScopeType::Property(v)
        | SchemaScopeType::Any(v)
        | SchemaScopeType::Form(v)
        | SchemaScopeType::Definition(v) => Some(v),
        SchemaScopeType::Reference(t) => Some(format!("\x1b[0;32m{t}\x1b[0m")),
        SchemaScopeType::Index(i) => Some(format!("{i}")),
    }
    .map(|s| s.replace('/', "~1"))
}
