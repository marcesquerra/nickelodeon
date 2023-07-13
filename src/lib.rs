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
            .add_root_etc()
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

    use serde::Deserialize;
    use serde::Serialize;

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

        #[test]
        fn first_file_found() {
            fn is_file(path: &PathBuf) -> bool {
                path.as_os_str().to_string_lossy().ends_with("_file")
            }

            let candidates = vec![
                PathBuf::from("file_is_not"),
                PathBuf::from("the_actual_file"),
                PathBuf::from("not_the_first_file"),
            ];

            let result = first_existing_config_impl(is_file, candidates);

            assert_eq!(result, Some(PathBuf::from("the_actual_file")));
        }
    }

    #[cfg(test)]
    mod all_location_candidates {
        use super::super::all_location_candidates;
        use super::super::all_location_candidates_impl;
        use std::io;
        use std::path::PathBuf;

        #[test]
        fn works() {
            if cfg!(windows) {
                // The logic that depends on the underlaying platform is implemented by
                // the [`config_finder`] crate, making the logic in [`nickelodeon`] platform
                // independent. On the other hand, testing this for Windows is difficult
                // and, if the tests pass on linux, unnecesary
                panic!("This test was not intended to run in Windows")
            }
            std::env::set_var("HOME", "/home/testuser");
            std::env::remove_var("XDG_CONFIG_HOME");
            fn pwd_mock() -> io::Result<PathBuf> {
                Ok(PathBuf::from("/projects/project_folder"))
            }
            let result = all_location_candidates_impl(pwd_mock, "some_app");
            let expected = vec![
                PathBuf::from("/projects/project_folder/.some_app/config.ncl"),
                PathBuf::from("/projects/project_folder/.some_app/config.nickel"),
                PathBuf::from("/home/testuser/.config/some_app/config.ncl"),
                PathBuf::from("/home/testuser/.config/some_app/config.nickel"),
                PathBuf::from("/etc/some_app/config.ncl"),
                PathBuf::from("/etc/some_app/config.nickel"),
            ];
            assert_eq!(result, expected);
        }

        #[test]
        fn wired_correctly() {
            std::env::set_var("HOME", "/home/testuser");
            std::env::remove_var("XDG_CONFIG_HOME");
            let result = all_location_candidates("some_app");
            let expected = 6;
            assert_eq!(result.len(), expected);
        }
    }

    #[cfg(test)]
    mod load {
        use crate::tests::TestConfiguration;

        use super::super::load;
        use std::io::Write;
        use tempfile::NamedTempFile;

        #[test]
        fn happy() {
            let mut ntf = NamedTempFile::new().unwrap();
            ntf.write_fmt(format_args!(
                "{}",
                r#"
                    {
                      test_value = "nick",
                    }
                "#,
            ))
            .unwrap();

            let result: TestConfiguration = load(ntf.path().to_path_buf()).unwrap();
            let expected = TestConfiguration {
                test_value: "nick".to_string(),
            };

            assert_eq!(result, expected);
        }
    }

    #[cfg(test)]
    mod load_configuration {
        use crate::tests::TestConfiguration;

        use super::super::load_configuration;
        use std::fs::{create_dir_all, File};
        use std::io::Write;
        use tempfile::tempdir;

        #[test]
        fn happy() {
            if cfg!(windows) {
                // The logic that depends on the underlaying platform is implemented by
                // the [`config_finder`] crate, making the logic in [`nickelodeon`] platform
                // independent. On the other hand, testing this for Windows is difficult
                // and, if the tests pass on linux, unnecesary
                panic!("This test was not intended to run in Windows")
            }

            let home_config_dir = tempdir().unwrap();
            let home_config_path = home_config_dir.path();
            let config_dir_path = home_config_path.join("some_app");
            create_dir_all(config_dir_path.clone()).unwrap();
            let config_file_path = config_dir_path.join("config.ncl");
            let mut conf_file = File::create(config_file_path).unwrap();
            conf_file
                .write_fmt(format_args!(
                    "{}",
                    r##"
                        {
                          test_value = "nick",
                        }
                    "##
                ))
                .unwrap();
            std::env::set_var("XDG_CONFIG_HOME", home_config_path.to_str().unwrap());

            let result: TestConfiguration = load_configuration("some_app", None).unwrap();
            let expected = TestConfiguration {
                test_value: "nick".to_string(),
            };

            assert_eq!(result, expected);
        }
    }

    #[derive(Serialize, Deserialize, Debug, Default, PartialEq)]
    struct TestConfiguration {
        pub test_value: String,
    }
}
