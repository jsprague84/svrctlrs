// Database models for the new unified schema

pub mod credential;
pub mod job_catalog;
pub mod job_run;
pub mod job_schedule;
pub mod job_template;
pub mod job_type;
pub mod notification;
pub mod server;
pub mod setting;
pub mod tag;
pub mod terminal_profile;

// Re-export all models for convenience
pub use credential::*;
pub use job_catalog::*;
pub use job_run::*;
pub use job_schedule::*;
pub use job_template::*;
pub use job_type::*;
pub use notification::*;
pub use server::*;
pub use setting::*;
pub use tag::*;
pub use terminal_profile::*;
