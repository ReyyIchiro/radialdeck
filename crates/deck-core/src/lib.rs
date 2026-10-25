//! Platform-independent domain logic: configuration, folder shortcuts,
//! search and persistence. No UI or Linux-specific code lives here.

pub mod config;
pub mod convert;
pub mod file_actions;
pub mod folder;
pub mod search;
pub mod store;
pub mod tools;

pub use config::{Config, Preferences, Theme, SCHEMA_VERSION};
pub use convert::{convert, Category, ConvertError, Failure, Outcome, Target, Tools};
pub use folder::{availability, Availability, Folder, FolderError, Project};
pub use search::filter;
pub use store::{default_config_path, Store, StoreError};
pub use tools::{ToolId, ToolOutcome, ToolRequest};
