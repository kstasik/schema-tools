use std::fmt;

use regex::Regex;

use crate::error::Error;

#[derive(Clone, Debug)]
pub enum SchemaNamingStrategy {
    Default,
}

#[derive(Clone, Debug)]
pub struct SchemaScope {
    scope: Vec<SchemaScopeType>,
    naming_strategy: SchemaNamingStrategy,
}

#[derive(Clone, Debug, PartialEq)]
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
            .last()
            .unwrap()
        {
            SchemaScopeType::Entity(name) => Ok(self.split(name)),
            SchemaScopeType::Property(last) | SchemaScopeType::Definition(last) => {
                let entity = self
                    .parts
                    .clone()
                    .into_iter()
                    .filter_map(|s| match s {
                        SchemaScopeType::Entity(t) => Some(t),
                        _ => None,
                    })
                    .last();

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

                let parts: Vec<String> = glued.iter().map(|s| self.split(s)).flatten().collect();
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
            .filter(|c| c.is_ascii_alphabetic() || c.is_ascii_alphanumeric())
            .collect::<String>();

        let result: Vec<String> = t
            .split(' ')
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>();

        result
    }
}

impl SchemaScope {
    pub fn default() -> Self {
        Self {
            scope: vec![],
            naming_strategy: SchemaNamingStrategy::Default,
        }
    }

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
            .unwrap_or_else(|| format!("{}", self))
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
        SchemaScopeType::Reference(t) => Some(format!("\x1b[0;32m{}\x1b[0m", t)),
        SchemaScopeType::Index(i) => Some(format!("{}", i)),
    }
    .map(|s| s.replace("/", "~1"))
}
