//! PVPC Cheap Backend Library
//!
//! This library provides the core functionality for the PVPC Cheap automation service,
//! including:
//! - Smart home device integrations (Meross, Home Assistant, Shelly)
//! - PVPC electricity price fetching from ESIOS API
//! - Automation rules engine
//! - User authentication and management

pub mod api;
pub mod db;
pub mod integrations;
pub mod lang;
pub mod models;
pub mod schema;
pub mod services;
pub mod time;
pub mod zones;
