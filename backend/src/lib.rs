//! PVPC Cheap Backend Library
//!
//! This library provides the core functionality for the PVPC Cheap automation service,
//! including:
//! - Smart home device integrations (Meross, and future providers)
//! - PVPC electricity price fetching from ESIOS API
//! - Automation rules engine
//! - User authentication and management

pub mod api;
pub mod db;
pub mod integrations;
pub mod models;
pub mod schema;
pub mod services;
