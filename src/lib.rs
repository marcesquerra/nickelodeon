use config_finder::ConfigDirs;
use nickel_lang::eval::cache::CacheImpl;
use nickel_lang::program::Program;
use nickel_lang::term::RichTerm;
use serde::Deserialize;
use std::path::PathBuf;

pub fn load_configuration<'a, T: Deserialize<'a> + Default>(
    app: &str,
    config_path_from_flag: Option<PathBuf>,
) -> Result<T> {
    if let Some(path) = config_path_from_flag.or(first_existing_config(app)) {
        load(path)
    } else {
        Ok(T::default())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    ConfigFileReadingError(String),
    NickelEvaluationError(nickel_lang::error::Error),
    RustDeserializationError(nickel_lang::deserialize::RustDeserializationError),
}

fn expand_names(mut pb1: PathBuf) -> Vec<PathBuf> {
    let mut pb2 = pb1.clone();
    pb1.push("config.ncl");
    pb2.push("config.nickel");
    vec![pb1, pb2]
}

fn expand_path_and_names(app: &str, pb0: &PathBuf) -> Vec<PathBuf> {
    let mut pb1 = pb0.clone();
    pb1.push(app);
    expand_names(pb1)
}

fn first_existing_config(app: &str) -> Option<PathBuf> {
    let mut buffer: Vec<PathBuf> = if let Ok(mut pwd_base) = std::env::current_dir() {
        pwd_base.push(format!(".{}", app));
        expand_names(pwd_base)
    } else {
        Vec::new()
    };

    buffer.append(
        &mut ConfigDirs::empty()
            .add_platform_config_dir()
            .paths()
            .into_iter()
            .flat_map(|pb0| expand_path_and_names(app, pb0))
            .collect(),
    );

    buffer.into_iter().find(|pb| pb.is_file())
}

fn load<'a, T: Deserialize<'a>>(path: PathBuf) -> Result<T> {
    let mut program: Program<CacheImpl> =
        Program::new_from_file(path).map_err(|e| Error::ConfigFileReadingError(e.to_string()))?;
    let rt: RichTerm = program
        .eval_full_for_export()
        .map(RichTerm::from)
        .map_err(Error::NickelEvaluationError)?;

    T::deserialize(rt).map_err(Error::RustDeserializationError)
}
