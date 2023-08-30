use std::{path::PathBuf, io::SeekFrom, collections::HashMap};

use tokio::{io::{AsyncWriteExt, AsyncReadExt, AsyncSeekExt, AsyncRead, AsyncSeek}, fs::{File, OpenOptions}};

use crate::types::{MediaType, SizedReference, CachedMedia};

pub const LATEST_VERSION: u64 = 1;

#[derive(Debug)]
pub struct IloveuTransactionsStore {
    path: PathBuf,
}

impl IloveuTransactionsStore {
    pub async fn open<P: Into<PathBuf>>(path: P) -> Result<IloveuTransactionsStore, tokio::io::Error> {
        let path = path.into();
        if !(tokio::fs::try_exists(&path).await?) {
            tokio::fs::create_dir(&path).await?;
            tokio::fs::File::create(path.join("version")).await?.write_u64(0).await?;
        }

        let mut iloveu_transactions_store = IloveuTransactionsStore {
            path,
        };

        iloveu_transactions_store.run_migrations().await?;

        Ok(iloveu_transactions_store)
    }

    async fn run_migrations(&mut self) -> Result<(), tokio::io::Error> {
        let mut version_file = tokio::fs::OpenOptions::new()
            .write(true).read(true).open(self.path.join("version")).await?;
        let last_version = version_file.read_u64().await?;
        
        if last_version != LATEST_VERSION {
            match last_version {
                0 => {
                    tokio::fs::File::create(self.path.join("transactions")).await?;
                },
                1 => {},
                _ => todo!("Unknown version")
            }
            version_file.seek(SeekFrom::Start(0)).await?;
            version_file.write_u64(LATEST_VERSION).await?;
        }

        Ok(())
    }

    pub async fn get_transactions_raw(&self) -> Result<File, tokio::io::Error> {
        File::open(self.path.join("transactions")).await
    }

    pub async fn add_tag(&mut self, name: &str) -> Result<(), tokio::io::Error> {
        let mut transactions_file = OpenOptions::new().append(true).open(self.path.join("transactions")).await?;
        transactions_file.write_u64(0).await?; // transaction type tag
        let name_bytes = name.as_bytes();
        transactions_file.write_u64(name_bytes.len() as u64).await?; // size of name
        transactions_file.write_all(name_bytes).await?; // name (utf8)
        
        Ok(())
    }

    pub async fn add_media(&mut self, title: &str, description: &str, tags_vec: &Vec<u64>, taken_datetime: f64, media_type: MediaType, filename: &str, file_bytes: &Vec<u8>)-> Result<SizedReference, tokio::io::Error> {
        let mut transactions_file = OpenOptions::new().append(true).open(self.path.join("transactions")).await?;
        transactions_file.write_u64(1).await?;

        let title_bytes = title.as_bytes();
        transactions_file.write_u64(title_bytes.len() as u64).await?;
        transactions_file.write_all(title_bytes).await?;

        let description_bytes = description.as_bytes();
        transactions_file.write_u64(description_bytes.len() as u64).await?;
        transactions_file.write_all(description_bytes).await?;

        let tags_count = tags_vec.len() as u64;
        transactions_file.write_u64(tags_count).await?;
        for tag_id in tags_vec {
            transactions_file.write_u64(*tag_id).await?;
        }

        transactions_file.write_f64(taken_datetime).await?;

        transactions_file.write_u64(match media_type {
            MediaType::Picture => 0,
            MediaType::Video => 1,
        }).await?;

        let filename_bytes = filename.as_bytes();
        transactions_file.write_u64(filename_bytes.len() as u64).await?;
        transactions_file.write_all(filename_bytes).await?;

        transactions_file.write_u64(file_bytes.len() as u64).await?;
        let file_offset = transactions_file.stream_position().await?;
        transactions_file.write_all(&file_bytes).await?;

        Ok(SizedReference {
            offset: file_offset,
            size: file_bytes.len() as u64
        })
    }
}

pub enum Transaction {
    AddTag(String),
}

#[derive(Debug)]
pub struct IloveuCache {
    next_tag_id: u64,
    tags: HashMap<u64, String>,
    next_media_id: u64,
    media: HashMap<u64, CachedMedia>,
}

impl IloveuCache {
    pub fn new() -> Self {
        Self {
            next_tag_id: 0,
            tags: HashMap::new(),
            next_media_id: 0,
            media: HashMap::new(),
        }
    }

    pub async fn run_raw_transactions<T: AsyncRead+AsyncSeek+Unpin>(&mut self, mut transaction_stream: T) -> Result<(), tokio::io::Error> {
        loop {
            match transaction_stream.read_u64().await {
                Ok(u) => match u {
                    0 => {
                        let length = transaction_stream.read_u64().await?;
                        let mut name_bytes = (0..length).map(|_| 0u8).collect::<Vec<u8>>();
                        transaction_stream.read_exact(name_bytes.as_mut_slice()).await?;
                        let name = String::from_utf8(name_bytes).unwrap();
                        self.tags.insert(self.next_tag_id, name);
                        self.next_tag_id += 1;
                    },
                    1 => {
                        let title_length = transaction_stream.read_u64().await?;
                        let mut title_bytes = (0..title_length).map(|_| 0u8).collect::<Vec<u8>>();
                        transaction_stream.read_exact(title_bytes.as_mut_slice()).await?;
                        let title = String::from_utf8(title_bytes).unwrap();

                        let description_length = transaction_stream.read_u64().await?;
                        let mut description_bytes = (0..description_length).map(|_| 0u8).collect::<Vec<u8>>();
                        transaction_stream.read_exact(description_bytes.as_mut_slice()).await?;
                        let description = String::from_utf8(description_bytes).unwrap();

                        let tags_count = transaction_stream.read_u64().await?;
                        let mut tags_vec: Vec<u64> = Vec::with_capacity(tags_count as usize);
                        for _ in 0..tags_count {
                            tags_vec.push(transaction_stream.read_u64().await?);
                        }

                        let taken_datetime = transaction_stream.read_f64().await?;

                        let media_type = match transaction_stream.read_u64().await? {
                            0 => MediaType::Picture,
                            1 => MediaType::Video,
                            _ => panic!("unknown media type")
                        };

                        let filename_length = transaction_stream.read_u64().await?;
                        let mut filename_bytes = (0..filename_length).map(|_| 0u8).collect::<Vec<u8>>();
                        transaction_stream.read_exact(filename_bytes.as_mut_slice()).await?;
                        let filename = String::from_utf8(filename_bytes).unwrap();

                        let compressed_file_length = transaction_stream.read_u64().await?;
                        let compressed_file_offset = transaction_stream.stream_position().await?;
                        transaction_stream.seek(SeekFrom::Current(compressed_file_length as i64)).await?;
                        let compressed_file_reference = SizedReference {
                            offset: compressed_file_offset,
                            size: compressed_file_length,
                        };

                        self.media.insert(self.next_media_id, CachedMedia {
                            title,
                            description,
                            tags_vec,
                            taken_datetime,
                            media_type,
                            filename,
                            file_reference: compressed_file_reference,
                        });

                        self.next_media_id += 1;
                    }
                    _ => {
                        return Err(tokio::io::Error::new(tokio::io::ErrorKind::Other, "unknown transaction type"))
                    }
                },
                Err(err) => match err.kind() {
                    tokio::io::ErrorKind::UnexpectedEof => return Ok(()),
                    _ => {
                        return Err(err)
                    }
                }
            }
        }
    }

    pub fn add_tag(&mut self, name: String) -> u64 {
        let tag_id = self.next_tag_id;

        self.tags.insert(tag_id, name);

        self.next_tag_id += 1;

        tag_id
    }

    pub fn get_tags(&self) -> &HashMap<u64, String> {
        &self.tags
    }

    pub fn add_media(&mut self, cached_media: CachedMedia) -> u64 {
        let media_id = self.next_media_id;

        self.media.insert(media_id, cached_media);

        self.next_media_id += 1;

        media_id
    }

    pub fn get_media(&self) -> &HashMap<u64, CachedMedia> {
        &self.media
    }
}