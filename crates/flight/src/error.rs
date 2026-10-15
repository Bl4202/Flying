//! Why content could not be turned into a flight model.

use core::fmt;

use crate::table::TableError;

/// A content entry the flight model cannot use. The baker rejects all of these, so they mean a
/// pack that bypassed it or a bug.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelError {
    /// A referenced entry is missing or of the wrong kind.
    Missing(&'static str),
    /// More of something than the model has fixed room for.
    TooMany {
        /// What.
        what: &'static str,
        /// How many there are.
        found: usize,
        /// How many fit.
        max: usize,
    },
    /// A table is malformed.
    Table {
        /// Which table.
        table: &'static str,
        /// What is wrong with it.
        error: TableError,
    },
    /// A value is outside what the model can work with.
    Invalid(&'static str),
}

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing(what) => write!(f, "missing {what}"),
            Self::TooMany { what, found, max } => {
                write!(f, "{found} {what}, at most {max} are supported")
            }
            Self::Table { table, error } => write!(f, "table {table}: {error}"),
            Self::Invalid(what) => write!(f, "invalid {what}"),
        }
    }
}

impl std::error::Error for ModelError {}

/// Adds the table's name to a [`TableError`].
pub(crate) fn table<T>(name: &'static str, r: Result<T, TableError>) -> Result<T, ModelError> {
    r.map_err(|error| ModelError::Table { table: name, error })
}
