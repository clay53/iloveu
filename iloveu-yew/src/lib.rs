#![feature(iter_intersperse)]
pub mod home;
pub mod add_media;
pub mod add_tag;

pub const API_ROOT: &'static str = std::env!("API_ROOT");

#[derive(Clone, Debug, PartialEq)]
pub struct HashedSessionIDBase64(pub String);