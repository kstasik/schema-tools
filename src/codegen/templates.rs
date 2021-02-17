use serde::Serialize;
use serde_json::Value;
use tera::Context;
use tera::Tera;
use walkdir::WalkDir;

use crate::{error::Error, tools};
use std::{
    collections::HashMap,
    ffi::OsStr,
    fs::File,
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::Command,
};
#[derive(Debug)]
pub struct Templates {
    pub list: Vec<Template>,
}

#[derive(Debug)]
pub enum Template {
    Models(ModelsTemplate),
    Endpoints(EndpointsTemplate),
    Static(StaticTemplate),
}

#[derive(Debug)]
pub struct EndpointsTemplate {
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

#[derive(Debug)]
pub struct StaticTemplate {
    absolute: PathBuf,
    relative: PathBuf,
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
    pub fn from(absolute: PathBuf, relative: PathBuf) -> Result<Self, Error> {
        if absolute.extension().and_then(OsStr::to_str) == Some("j2") {
            Template::parse(absolute, relative)
        } else {
            Ok(Template::Static(StaticTemplate { absolute, relative }))
        }
    }

    fn parse(absolute: PathBuf, relative: PathBuf) -> Result<Self, Error> {
        let mut reader =
            BufReader::new(File::open(absolute).map_err(|_| Error::CodegenFileSkipped)?);

        let mut first_line = String::new();
        reader
            .read_line(&mut first_line)
            .map_err(|_| Error::CodegenFileSkipped)?;

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
                "endpoints" => EndpointsTemplate::from(relative, &params),
                "models" => ModelsTemplate::from(relative, &params),
                _ => Err(Error::CodegenFileHeaderRequired("type".to_string())),
            })
            .unwrap()
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

        Ok(Template::Endpoints(Self {
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
        if self
            .condition
            .as_ref()
            .map(|s| s.check(container))
            .unwrap_or(true)
        {
            let openapi = openapi.clone().set_content_type(&self.content_type);

            process_render(
                tera,
                openapi,
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
        let target = PathBuf::from(format!(
            "{}/{}",
            target_dir,
            self.relative.to_string_lossy()
        ));

        log::info!("Copying: {:?}", target);

        let mut directory = target.clone();
        directory.pop();

        std::fs::create_dir_all(directory).map_err(|e| Error::CodegenFileError(e.to_string()))?;

        std::fs::copy(self.absolute.clone(), target.clone())
            .map(|_| ())
            .map_err(|e| Error::CodegenFileError(e.to_string()))?;

        Ok(vec![target.to_string_lossy().to_string()])
    }
}

pub fn get(templates_dir: &str) -> Result<Templates, Error> {
    let list = WalkDir::new(templates_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| !e.file_type().is_dir())
        .filter_map(|f| {
            match Template::from(
                f.clone().into_path(),
                f.into_path()
                    .strip_prefix(templates_dir)
                    .unwrap()
                    .to_path_buf(),
            ) {
                Ok(t) => Some(Ok(t)),
                Err(e) => match e {
                    Error::CodegenFileSkipped => None,
                    e => Some(Err(e)),
                },
            }
        })
        .collect::<Result<Vec<_>, Error>>()?;

    if list.is_empty() {
        return Err(Error::CodegenTemplatesDirectoryError(
            templates_dir.to_string(),
        ));
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
    ctx.insert(
        "options".to_string(),
        &serde_json::to_value(container.options.clone()).unwrap(),
    );

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
