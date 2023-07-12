#![deny(clippy::all)]
#![allow(clippy::blanket_clippy_restriction_lints)]
#![deny(clippy::pedantic)]
#![deny(clippy::restriction)]
#![deny(clippy::nursery)]
#![deny(clippy::cargo)]
#![allow(clippy::multiple_crate_versions)]
#![allow(clippy::implicit_return)]
#![allow(clippy::missing_inline_in_public_items)]
#![allow(clippy::std_instead_of_core)]
#![allow(clippy::missing_docs_in_private_items)]
#![allow(clippy::question_mark_used)]

use config_finder::ConfigDirs;
use nickel_lang_core::eval::cache::CacheImpl;
use nickel_lang_core::program::Program;
use nickel_lang_core::term::RichTerm;
use serde::Deserialize;
use std::io;
use std::path::Path;
use std::path::PathBuf;

/// # Errors
///
/// Will return `Err` if the found config file can't be read, evaluated or if it
/// doesn't match the deserialization contract for `T`
pub fn load_configuration<'de, T: Deserialize<'de> + Default>(
    app: &str,
    config_path_from_flag: Option<PathBuf>,
) -> Result<T> {
    config_path_from_flag
        .or_else(|| first_existing_config(app))
        .map_or_else(|| Ok(T::default()), |path| load(path))
}

/// A specialized [`Result`] type for nickelodeon operations.
///
/// This type is used in [`nickelodeon`] for reporting the location,
/// loading, evaluation and deserialization of configuration files
/// written in Nickel
pub type Result<T> = std::result::Result<T, Error>;

/// Describes everything that can go wrong loading [`ConfigFileReadingError`],
/// evaluating [`NickelEvaluationError`] or deserializing [`RustDeserializationError`]
/// Nickel configuration files
#[allow(clippy::exhaustive_enums)]
#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// Something went wrong reading the file
    ConfigFileReadingError(String),

    /// Something went wrong evaluating the nickel program (i.e. running the nickel code)
    NickelEvaluationError(nickel_lang_core::error::Error),

    /// Something went wrong converting the resulting nickel data into the requested shape
    RustDeserializationError(nickel_lang_core::deserialize::RustDeserializationError),
}

/// Given a base path, returns the two possible names the configuration file might have
fn expand_names(mut pb1: PathBuf) -> Vec<PathBuf> {
    let mut pb2 = pb1.clone();
    pb1.push("config.ncl");
    pb2.push("config.nickel");
    vec![pb1, pb2]
}

/// Given a base path, and an application codename, returns the two possible locations (e.g. `app/config.ncl` and
/// `app/config.nickel`) where the configuration file might be located
fn expand_path_and_names(app: &str, pb0: &Path) -> Vec<PathBuf> {
    let mut pb1 = pb0.to_path_buf();
    pb1.push(app);
    expand_names(pb1)
}

fn all_location_candidates(app: &str) -> Vec<PathBuf> {
    all_location_candidates_impl(std::env::current_dir, app)
}

fn all_location_candidates_impl<F>(pwd: F, app: &str) -> Vec<PathBuf>
where
    F: Fn() -> io::Result<PathBuf>,
{
    let mut buffer: Vec<PathBuf> = pwd().map_or_else(
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

    buffer
}

/// Goes through all the locations that the configuration file for an app
/// with the codename [`app`] could be located and return the full path of
/// the first one that actually exist and is a file
fn first_existing_config(app: &str) -> Option<PathBuf> {
    first_existing_config_impl(|pb| pb.is_file(), all_location_candidates(app))
}

/// Goes through all the locations that the configuration file for an app
/// with the codename [`app`] could be located and return the full path of
/// the first one that actually exist and is a file.
///
/// This implementation uses the `P` predicate to decide if a path exists.
/// This approach is used to facilitate testing.
fn first_existing_config_impl<P>(is_file: P, candidates: Vec<PathBuf>) -> Option<PathBuf>
where
    P: FnMut(&PathBuf) -> bool,
{
    candidates.into_iter().find(is_file)
}

/// Loads, evaluates and deserializes the data in the file located at [`path`].
///
/// # Errors
///
/// Will return `Err` if the file can't be read, evaluated or if it doesn't match
/// the deserialization contract for `T`
fn load<'de, T: Deserialize<'de>>(path: PathBuf) -> Result<T> {
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

    #[cfg(test)]
    mod first_existing_config {
        use super::super::first_existing_config;
        use super::super::first_existing_config_impl;
        use std::path::PathBuf;

        #[test]
        fn nothing_found() {
            let result = first_existing_config("this_app_does_not_exist");
            assert_eq!(result, None);
        }

        #[test]
        fn one_file_exists() {
            fn is_file(path: &PathBuf) -> bool {
                path.as_os_str().to_string_lossy().ends_with("_file")
            }

            let candidates = vec![
                PathBuf::from("file_is_not"),
                PathBuf::from("the_actual_file"),
            ];

            let result = first_existing_config_impl(is_file, candidates);

            assert_eq!(result, Some(PathBuf::from("the_actual_file")));
        }
    }
}
