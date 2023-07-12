#![deny(clippy::all)]
#![warn(clippy::pedantic)]
// #![warn(clippy::restriction)]
#![warn(clippy::nursery)]
#![warn(clippy::cargo)]
#![allow(clippy::multiple_crate_versions)]

use config_finder::ConfigDirs;
use nickel_lang_core::eval::cache::CacheImpl;
use nickel_lang_core::program::Program;
use nickel_lang_core::term::RichTerm;
use serde::Deserialize;
use std::path::Path;
use std::path::PathBuf;

/// # Errors
///
/// Will return `Err` if the found config file can't be read, evaluated or if it
/// doesn't match the contract for `T`
pub fn load_configuration<'a, T: Deserialize<'a> + Default>(
    app: &str,
    config_path_from_flag: Option<PathBuf>,
) -> Result<T> {
    config_path_from_flag
        .or_else(|| first_existing_config(app))
        .map_or_else(|| Ok(T::default()), |path| load(path))
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    ConfigFileReadingError(String),
    NickelEvaluationError(nickel_lang_core::error::Error),
    RustDeserializationError(nickel_lang_core::deserialize::RustDeserializationError),
}

fn expand_names(mut pb1: PathBuf) -> Vec<PathBuf> {
    let mut pb2 = pb1.clone();
    pb1.push("config.ncl");
    pb2.push("config.nickel");
    vec![pb1, pb2]
}

fn expand_path_and_names(app: &str, pb0: &Path) -> Vec<PathBuf> {
    let mut pb1 = pb0.to_path_buf();
    pb1.push(app);
    expand_names(pb1)
}

fn first_existing_config(app: &str) -> Option<PathBuf> {
    first_existing_config_impl(|pb| pb.is_file(), app)
}

fn first_existing_config_impl<P>(p: P, app: &str) -> Option<PathBuf>
where
    P: FnMut(&PathBuf) -> bool,
{
    let mut buffer: Vec<PathBuf> = std::env::current_dir().map_or_else(
        |_| Vec::new(),
        |mut pwd_base| {
            pwd_base.push(format!(".{app}"));
            expand_names(pwd_base)
        },
    );

    buffer.append(
        &mut ConfigDirs::empty()
            .add_platform_config_dir()
            .paths()
            .iter()
            .flat_map(|pb0| expand_path_and_names(app, pb0))
            .collect(),
    );

    buffer.into_iter().find(p)
}

fn load<'a, T: Deserialize<'a>>(path: PathBuf) -> Result<T> {
    let mut program: Program<CacheImpl> = Program::new_from_file(path, std::io::stderr())
        .map_err(|e| Error::ConfigFileReadingError(e.to_string()))?;
    let rt: RichTerm = program
        .eval_full_for_export()
        .map(RichTerm::from)
        .map_err(Error::NickelEvaluationError)?;

    T::deserialize(rt).map_err(Error::RustDeserializationError)
}

#[cfg(test)]
mod tests {

    #[cfg(test)]
    mod expand_names {
        use super::super::expand_names;
        use std::path::PathBuf;

        #[test]
        fn happy_path() {
            let result = expand_names(PathBuf::from("/tmp"));
            let expected: Vec<PathBuf> = vec![
                PathBuf::from("/tmp/config.ncl"),
                PathBuf::from("/tmp/config.nickel"),
            ];
            assert_eq!(result, expected);
        }

        #[test]
        fn trivial() {
            let result = expand_names(PathBuf::new());
            let expected: Vec<PathBuf> =
                vec![PathBuf::from("config.ncl"), PathBuf::from("config.nickel")];
            assert_eq!(result, expected);
        }
    }

    #[cfg(test)]
    mod expand_path_and_names {
        use super::super::expand_path_and_names;
        use std::path::PathBuf;

        #[test]
        fn happy_path() {
            let result = expand_path_and_names("app", &PathBuf::from("/tmp"));
            let expected: Vec<PathBuf> = vec![
                PathBuf::from("/tmp/app/config.ncl"),
                PathBuf::from("/tmp/app/config.nickel"),
            ];
            assert_eq!(result, expected);
        }

        #[test]
        fn blank_app_name() {
            let result = expand_path_and_names("", &PathBuf::from("/tmp"));
            let expected: Vec<PathBuf> = vec![
                PathBuf::from("/tmp/config.ncl"),
                PathBuf::from("/tmp/config.nickel"),
            ];
            assert_eq!(result, expected);
        }

        #[test]
        fn trivial() {
            let result = expand_path_and_names("app", &PathBuf::new());
            let expected: Vec<PathBuf> = vec![
                PathBuf::from("app/config.ncl"),
                PathBuf::from("app/config.nickel"),
            ];
            assert_eq!(result, expected);
        }

        #[test]
        fn trivial_with_blank_app_name() {
            let result = expand_path_and_names("", &PathBuf::new());
            let expected: Vec<PathBuf> =
                vec![PathBuf::from("config.ncl"), PathBuf::from("config.nickel")];
            assert_eq!(result, expected);
        }
    }
}
