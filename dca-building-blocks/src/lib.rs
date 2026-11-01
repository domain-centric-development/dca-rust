//! Building blocks of Domain-Centric Architecture for Rust.
//!
//! Each trait names a role. A type takes the role with `impl <Role> for <Type>`, a trait with a
//! supertrait; the rule suite reads the roles from the source. The crate has no dependencies, so a
//! domain crate can depend on it without pulling in anything else.

#![forbid(unsafe_code)]

pub mod application;
pub mod ddd;
pub mod hexagonal;
