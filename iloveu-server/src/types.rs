use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
pub enum MediaType {
    Picture,
    Video,
}

impl MediaType {
    
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct SizedReference {
    pub offset: u64,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CachedMedia {
    pub title: String,
    pub description: String,
    pub tags_vec: Vec<u64>,
    pub taken_datetime: f64,
    pub media_type: MediaType,
    pub filename: String,
    pub file_reference: SizedReference,
}