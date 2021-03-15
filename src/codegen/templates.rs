use serde::Serialize;
use serde_json::Value;
use tera::Context;
use tera::Tera;

use crate::{discovery::Discovered, error::Error, tools};
use std::{collections::HashMap, fs::File, io::Write, path::PathBuf, process::Command};

use super::openapi::Openapi;

#[derive(Debug)]
pub struct Templates {
    pub list: Vec<Template>,
}

#[derive(Debug)]
pub enum Template {
    Models(ModelsTemplate),
    Endpoints(EndpointsTemplate),
    Tags(TagsTemplate),
    Static(StaticTemplate),
}

#[derive(Debug)]
pub struct EndpointsTemplate {
    relative: PathBuf,
    filename: Filename,
    content_type: String,
    condition: Option<Condition>,
    group_by: GroupBy,
}

#[derive(Debug)]
pub struct TagsTemplate {
    relative: PathBuf,
    filename: Filename,
    content_type: String,
    condition: Option<Condition>,
}

#[derive(Debug)]
pub struct ModelsTemplate {
    relative: PathBuf,
    filename: Filename,
    condition: Option<Condition>,
}

#[derive(Debug)]
pub struct Condition {
    pub kv: String,
}

#[derive(Debug, Default)]
pub struct GroupBy {
    pub kind: Option<String>,
}
#[derive(Serialize)]
pub struct TagContainer {
    tag: String,
    endpoints: Vec<super::openapi::Endpoint>,
}

pub trait Group {
    fn process(&self, openapi: &mut Openapi, container: &mut super::CodegenContainer);
}

#[derive(Debug)]
pub struct StaticTemplate {
    relative: String,
    path: PathBuf,
}
#[derive(PartialEq, Debug)]
pub enum TemplateType {
    Models,
    Endpoints,
}

#[derive(Debug, Clone)]
pub struct Filename {
    filename: String,
}

impl Filename {
    pub fn from(filename: String) -> Self {
        Self { filename }
    }

    pub fn resolve(&self, container: &super::CodegenContainer) -> Result<String, Error> {
        tools::fill_parameters(&self.filename, container)
    }
}

impl Condition {
    pub fn from(kv: &str) -> Result<Self, Error> {
        Ok(Self { kv: kv.to_string() })
    }

    pub fn check(&self, container: &super::CodegenContainer) -> bool {
        tools::fill_parameters(&self.kv, container)
            .map(|s| {
                let parts = s.split(':').collect::<Vec<&str>>();
                if let [left, right] = parts[..] {
                    left == right
                } else {
                    true
                }
            })
            .unwrap_or(false)
    }
}

impl GroupBy {
    pub fn from(group_by: &str) -> Result<Self, Error> {
        if group_by != "tag" {
            Err(Error::CodegenNotAllowedGroupBy(group_by.to_string()))
        } else {
            Ok(Self {
                kind: Some(group_by.to_string()),
            })
        }
    }

    pub fn split(&self, openapi: &Openapi) -> impl IntoIterator<Item = impl Group> {
        match &self.kind {
            Some(_) => TagGroup::produce(openapi)
                .into_iter()
                .map(GroupType::TagGroup)
                .collect::<Vec<_>>(),
            None => vec![GroupType::NoGroup],
        }
    }
}
pub struct TagGroup {
    tag: String,
}

impl Group for TagGroup {
    fn process(&self, openapi: &mut Openapi, container: &mut super::CodegenContainer) {
        container.data.insert(
            "tag".to_string(),
            Value::String(self.tag.clone().to_lowercase()),
        );

        openapi
            .endpoints
            .retain(|s| s.get_tags().contains(&self.tag));
    }
}

impl TagGroup {
    pub fn produce(openapi: &Openapi) -> Vec<TagGroup> {
        let mut tags = openapi.endpoints.iter().fold(vec![], |mut acc, item| {
            acc.append(&mut item.get_tags().clone());
            acc
        });

        tags.sort();
        tags.dedup();

        tags.iter()
            .map(|t| TagGroup { tag: t.clone() })
            .collect::<Vec<_>>()
    }

    pub fn filter(&self, endpoints: &[super::openapi::Endpoint]) -> Vec<super::openapi::Endpoint> {
        endpoints
            .iter()
            .filter(|e| e.get_tags().contains(&self.tag))
            .cloned()
            .collect()
    }
}

pub enum GroupType {
    TagGroup(TagGroup),
    NoGroup,
}

impl Group for GroupType {
    fn process(&self, openapi: &mut Openapi, container: &mut super::CodegenContainer) {
        match &self {
            Self::TagGroup(t) => t.process(openapi, container),
            Self::NoGroup => {}
        }
    }
}

impl Templates {
    pub fn includes(&self, types: &[TemplateType]) -> bool {
        self.list
            .iter()
            .filter_map(|t| match *t {
                Template::Models(_) => Some(TemplateType::Models),
                Template::Endpoints(_) => Some(TemplateType::Endpoints),
                _ => None,
            })
            .filter(|f| types.contains(&f))
            .count()
            > 0
    }
}

impl Template {
    fn from_static(relative: String, path: PathBuf) -> Self {
        Template::Static(StaticTemplate { relative, path })
    }

    fn from_content(relative: String, content: String) -> Result<Self, Error> {
        let first = content.lines().next();

        if let Some(line) = first {
            let mut first_line = line.to_string();

            let last_hash = first_line
                .char_indices()
                .find(|&(_, c)| c != '#')
                .map_or(0, |(idx, _)| idx);
            first_line = first_line[last_hash..].trim().to_string();

            if !first_line.starts_with("{# ") {
                return Err(Error::CodegenFileSkipped);
            }

            let params = super::format(&first_line.trim_matches(&['{', '}', '#', ' '] as &[_]))?;

            params
                .get("type")
                .ok_or_else(|| Error::CodegenFileHeaderRequired("type".to_string()))?
                .as_str()
                .map(|type_| match type_ {
                    "endpoints" => EndpointsTemplate::from(PathBuf::from(relative), &params),
                    "models" => ModelsTemplate::from(PathBuf::from(relative), &params),
                    "tags" => TagsTemplate::from(PathBuf::from(relative), &params),
                    _ => Err(Error::CodegenFileHeaderRequired("type".to_string())),
                })
                .unwrap()
        } else {
            Err(Error::CodegenFileSkipped)
        }
    }

    pub fn format(&self, command: &str, files: Vec<String>) -> Result<(), Error> {
        let parts = crate::tools::ArgumentsExtractor::new(command).collect::<Vec<String>>();

        for file in files {
            let mut cmd = Command::new(parts.get(0).unwrap());
            for i in 1..parts.len() {
                cmd.arg(parts.get(i).unwrap());
            }

            let output = cmd
                .arg(file)
                .output()
                .map_err(Error::CodegenFormattingError)?;

            if !output.status.success() {
                return Err(Error::CodegenFormattingCommandError(
                    String::from_utf8_lossy(&output.stderr).to_string(),
                ));
            }
        }

        Ok(())
    }
}

impl EndpointsTemplate {
    pub fn from(relative: PathBuf, config: &HashMap<&str, Value>) -> Result<Template, Error> {
        let filename = Filename::from(
            config
                .get("filename")
                .ok_or_else(|| Error::CodegenFileHeaderRequired("filename".to_string()))?
                .as_str()
                .unwrap()
                .to_string(),
        );

        let content_type = config
            .get("content_type")
            .map(|s| s.as_str().unwrap().to_string())
            .unwrap_or_else(|| "application/json".to_string());

        let condition = config
            .get("if")
            .map(|s| Condition::from(&s.as_str().unwrap()))
            .map_or(Ok(None), |v| v.map(Some))?;

        let group_by = config
            .get("group_by")
            .map(|s| GroupBy::from(&s.as_str().unwrap()))
            .unwrap_or_else(|| Ok(GroupBy::default()))?;

        Ok(Template::Endpoints(Self {
            relative,
            filename,
            content_type,
            condition,
            group_by,
        }))
    }

    pub fn render(
        &self,
        tera: &Tera,
        target_dir: &str,
        openapi: &super::openapi::Openapi,
        container: &super::CodegenContainer,
    ) -> Result<Vec<String>, Error> {
        let mut result = vec![];

        for group in self.group_by.split(openapi) {
            // prepare per group structures
            let mut openapi = openapi.clone().set_content_type(&self.content_type);
            let mut container = container.clone();

            // process group
            group.process(&mut openapi, &mut container);

            if self
                .condition
                .as_ref()
                .map(|s| s.check(&container))
                .unwrap_or(true)
            {
                // render
                result.append(&mut process_render(
                    tera,
                    openapi,
                    PathBuf::from(format!(
                        "{}/{}",
                        target_dir,
                        self.filename.resolve(&container)?
                    )),
                    self.relative.clone(),
                    &container,
                )?)
            } else {
                log::info!("Template skipped due to condition: {:?}", self.relative);
            }
        }

        Ok(result)
    }
}

impl TagsTemplate {
    pub fn from(relative: PathBuf, config: &HashMap<&str, Value>) -> Result<Template, Error> {
        let filename = Filename::from(
            config
                .get("filename")
                .ok_or_else(|| Error::CodegenFileHeaderRequired("filename".to_string()))?
                .as_str()
                .unwrap()
                .to_string(),
        );

        let content_type = config
            .get("content_type")
            .map(|s| s.as_str().unwrap().to_string())
            .unwrap_or_else(|| "application/json".to_string());

        let condition = config
            .get("if")
            .map(|s| Condition::from(&s.as_str().unwrap()))
            .map_or(Ok(None), |v| v.map(Some))?;

        Ok(Template::Tags(Self {
            relative,
            filename,
            content_type,
            condition,
        }))
    }

    pub fn render(
        &self,
        tera: &Tera,
        target_dir: &str,
        openapi: &super::openapi::Openapi,
        container: &super::CodegenContainer,
    ) -> Result<Vec<String>, Error> {
        let groups = TagGroup::produce(openapi);

        let mut tags: Vec<TagContainer> = vec![];
        let mut processed = openapi.clone().set_content_type(&self.content_type);
        let mut container = container.clone();

        for group in groups {
            tags.push(TagContainer {
                tag: group.tag.clone().to_lowercase(),
                endpoints: group.filter(&openapi.endpoints),
            })
        }

        processed.endpoints = vec![];

        container
            .data
            .insert("tags".to_string(), serde_json::to_value(tags).unwrap());

        if self
            .condition
            .as_ref()
            .map(|s| s.check(&container))
            .unwrap_or(true)
        {
            // render
            process_render(
                tera,
                processed,
                PathBuf::from(format!(
                    "{}/{}",
                    target_dir,
                    self.filename.resolve(&container)?
                )),
                self.relative.clone(),
                &container,
            )
        } else {
            log::info!("Template skipped due to condition: {:?}", self.relative);
            Ok(vec![])
        }
    }
}

impl ModelsTemplate {
    pub fn from(relative: PathBuf, config: &HashMap<&str, Value>) -> Result<Template, Error> {
        let filename = Filename::from(
            config
                .get("filename")
                .ok_or_else(|| Error::CodegenFileHeaderRequired("filename".to_string()))?
                .as_str()
                .unwrap()
                .to_string(),
        );

        let condition = config
            .get("if")
            .map(|s| Condition::from(&s.as_str().unwrap()))
            .map_or(Ok(None), |v| v.map(Some))?;

        Ok(Template::Models(Self {
            relative,
            filename,
            condition,
        }))
    }

    pub fn render(
        &self,
        tera: &Tera,
        target_dir: &str,
        models: &super::jsonschema::ModelContainer,
        container: &super::CodegenContainer,
    ) -> Result<Vec<String>, Error> {
        if self
            .condition
            .as_ref()
            .map(|s| s.check(container))
            .unwrap_or(true)
        {
            process_render(
                tera,
                models,
                PathBuf::from(format!(
                    "{}/{}",
                    target_dir,
                    self.filename.resolve(container)?
                )),
                self.relative.clone(),
                container,
            )
        } else {
            log::info!("Template skipped due to condition: {:?}", self.relative);

            Ok(vec![])
        }
    }
}

impl StaticTemplate {
    pub fn copy(&self, target_dir: &str) -> Result<Vec<String>, Error> {
        let target = PathBuf::from(format!("{}/{}", target_dir, self.relative));

        log::info!("Copying: {:?}", target);

        let mut directory = target.clone();
        directory.pop();

        std::fs::create_dir_all(directory).map_err(|e| Error::CodegenFileError(e.to_string()))?;

        std::fs::copy(self.path.clone(), target.clone())
            .map(|_| ())
            .map_err(|e| Error::CodegenFileError(e.to_string()))?;

        Ok(vec![target.to_string_lossy().to_string()])
    }
}

pub fn get(discovered: Discovered) -> Result<Templates, Error> {
    let mut list: Vec<Template> = vec![];

    for (relative, content) in discovered.templates {
        let result = Template::from_content(relative.clone(), content);

        match result {
            Ok(template) => {
                list.push(template);
            }
            Err(err) => match err {
                Error::CodegenFileSkipped => {
                    log::trace!("file skipped: {}", relative);
                    continue;
                }
                e => return Err(e),
            },
        }
    }

    for (relative, path) in discovered.files {
        list.push(Template::from_static(relative, path))
    }

    if list.is_empty() {
        return Err(Error::CodegenTemplatesDirectoryError);
    }

    Ok(Templates { list })
}

fn process_render(
    tera: &Tera,
    data: (impl Serialize + Clone),
    target: PathBuf,
    relative: PathBuf,
    container: &super::CodegenContainer,
) -> Result<Vec<String>, Error> {
    let mut ctx = Context::from_serialize(serde_json::to_value(data).unwrap()).unwrap();

    let data = serde_json::to_value(container).unwrap();
    for (key, value) in data.as_object().unwrap() {
        ctx.insert(key, value);
    }

    let result = tera
        .render(&relative.to_string_lossy(), &ctx)
        .map_err(Error::CodegenTemplateError)?;

    if result.trim().is_empty() {
        return Ok(vec![]);
    }

    log::info!("Rendering: {:?}", target);

    let mut directory = target.clone();
    directory.pop();

    std::fs::create_dir_all(directory).map_err(|e| Error::CodegenFileError(e.to_string()))?;

    let mut file =
        File::create(target.clone()).map_err(|e| Error::CodegenFileError(e.to_string()))?;

    file.write_all(result.as_bytes())
        .map_err(|e| Error::CodegenFileError(e.to_string()))?;

    Ok(vec![target.to_string_lossy().to_string()])
}
