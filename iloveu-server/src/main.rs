use std::{sync::Arc, pin::Pin, task::Poll};

use actix_cors::Cors;
use actix_multipart::Multipart;
use actix_web::{HttpServer, App, web::{self, Bytes}, get, post, HttpRequest, HttpResponse, http::header::{self, HeaderValue, ContentEncoding}};
use base64::Engine;
use clap::Parser;
use iloveu_server::{db::{IloveuTransactionsStore, IloveuCache}, session::{SessionManager, HashedSessionID}, types::{MediaType, CachedMedia}};
use tokio::{sync::RwLock, io::{AsyncWriteExt, AsyncSeekExt, AsyncReadExt}, fs::File};
use futures_util::{TryStreamExt};
use tokio_util::io::ReaderStream;

struct Config {
    password: Arc<String>,
}

#[derive(Debug, Clone)]
struct ActixTransactions(Arc<RwLock<IloveuTransactionsStore>>);

#[derive(Debug, Clone)]
struct ActixCache(Arc<RwLock<IloveuCache>>);

#[derive(Debug, Clone)]
struct ActixSessionManager(Arc<RwLock<SessionManager>>);

#[post("/login")]
async fn login(config: web::Data<Config>, sessions: web::Data<ActixSessionManager>, password: String) -> Result<Vec<u8>, actix_web::Error> {
    if password == *config.password {
        Ok(sessions.0.write().await.new_session().to_vec())
    } else {
        Err(actix_web::error::ErrorUnauthorized("invalid password"))
    }
}

#[post("/add_tag")]
async fn add_tag(sessions: web::Data<ActixSessionManager>, req: HttpRequest, cache: web::Data<ActixCache>, transactions: web::Data<ActixTransactions>, name: String) -> Result<Vec<u8>, actix_web::Error> {
    let authorization_header_value = req.headers().get("AUTHORIZATION").ok_or(actix_web::error::ErrorBadRequest("missing AUTHORIZATION header"))?;
    if sessions.0.read().await.validate_session(&HashedSessionID::clone_from_slice(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(authorization_header_value).map_err(|e| {
            actix_web::error::ErrorInternalServerError(format!("failed to decode base64: {}", e))
        })?))
    {
        transactions.0.write().await.add_tag(&name).await?;
        let id = cache.0.write().await.add_tag(name);
        Ok(id.to_be_bytes().to_vec())
    } else {
        Err(actix_web::error::ErrorUnauthorized("invalid session"))
    }
}

#[get("/tags")]
async fn tags(sessions: web::Data<ActixSessionManager>, req: HttpRequest, cache: web::Data<ActixCache>) -> Result<String, actix_web::Error> {
    let authorization_header_value = req.headers().get("AUTHORIZATION").ok_or(actix_web::error::ErrorBadRequest("missing AUTHORIZATION header"))?;
    if sessions.0.read().await.validate_session(&HashedSessionID::clone_from_slice(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(authorization_header_value).map_err(|e| {
            actix_web::error::ErrorInternalServerError(format!("failed to decode base64: {}", e))
        })?))
    {
        Ok(serde_json::to_string(cache.0.read().await.get_tags())?)
    } else {
        Err(actix_web::error::ErrorUnauthorized("invalid session"))
    }
}

#[post("/add_media")]
async fn add_media(sessions: web::Data<ActixSessionManager>, req: HttpRequest, cache: web::Data<ActixCache>, transactions: web::Data<ActixTransactions>, mut multipart: Multipart) -> Result<Vec<u8>, actix_web::Error> {
    let authorization_header_value = req.headers().get("AUTHORIZATION").ok_or(actix_web::error::ErrorBadRequest("missing AUTHORIZATION header"))?;
    if sessions.0.read().await.validate_session(&HashedSessionID::clone_from_slice(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(authorization_header_value).map_err(|e| {
            actix_web::error::ErrorInternalServerError(format!("failed to decode base64: {}", e))
        })?))
    {
        let mut title_field = multipart.try_next().await?.ok_or(actix_web::error::ErrorBadRequest("Missing title"))?;
        let mut title_bytes = Vec::new();
        while let Some(chunk) = title_field.try_next().await? {
            title_bytes.write_all(&chunk).await?;
        }
        let title = String::from_utf8(title_bytes).map_err(|e| actix_web::error::ErrorInternalServerError(format!("Failed to convert title bytes to UTF8 String: {}", e)))?;
        drop(title_field);

        let mut description_field = multipart.try_next().await?.ok_or(actix_web::error::ErrorBadRequest("Missing description"))?;
        let mut description_bytes = Vec::new();
        while let Some(chunk) = description_field.try_next().await? {
            description_bytes.write_all(&chunk).await?;
        }
        let description = String::from_utf8(description_bytes).map_err(|e| actix_web::error::ErrorInternalServerError(format!("Failed to convert description bytes to UTF8 String: {}", e)))?;
        drop(description_field);
        
        let mut tags_field = multipart.try_next().await?.ok_or(actix_web::error::ErrorBadRequest("Missing tags"))?;
        let mut tags_bytes = Vec::new();
        while let Some(chunk) = tags_field.try_next().await? {
            tags_bytes.write_all(&chunk).await?;
        }
        let tags_vec: Vec<u64> = serde_json::from_slice(tags_bytes.as_slice())?;
        drop(tags_field);

        let mut taken_datetime_field = multipart.try_next().await?.ok_or(actix_web::error::ErrorBadRequest("Missing taken_datetime"))?;
        let mut taken_datetime_bytes = Vec::new();
        while let Some(chunk) = taken_datetime_field.try_next().await? {
            taken_datetime_bytes.write_all(&chunk).await?;
        }
        let taken_datetime: f64 = serde_json::from_slice(taken_datetime_bytes.as_slice())?;
        drop(taken_datetime_field);

        let mut media_type_field = multipart.try_next().await?.ok_or(actix_web::error::ErrorBadRequest("Missing taken_datetime"))?;
        let mut media_type_bytes = Vec::new();
        while let Some(chunk) = media_type_field.try_next().await? {
            media_type_bytes.write_all(&chunk).await?;
        }
        let media_type = match media_type_bytes.as_slice() {
            b"picture" => Ok(MediaType::Picture),
            b"video" => Ok(MediaType::Video),
            _ => Err(actix_web::error::ErrorBadRequest("Unknown media type"))
        }?;
        drop(media_type_field);
        
        let mut file_field = multipart.try_next().await?.ok_or(actix_web::error::ErrorBadRequest("Missing file"))?;
        let filename = file_field.content_disposition().get_filename().ok_or(actix_web::error::ErrorBadRequest("Missing filename on file"))?.to_string();
        let mut file_bytes = Vec::new();
        while let Some(chunk) = file_field.try_next().await? {
            file_bytes.write_all(&chunk).await?;
        }
        drop(file_field);

        let file_reference = transactions.0.write().await.add_media(&title, &description, &tags_vec, taken_datetime, media_type, &filename, &file_bytes).await?;
        let media_id = cache.0.write().await.add_media(CachedMedia {
            title,
            description,
            tags_vec,
            taken_datetime,
            media_type,
            filename,
            file_reference,
        });

        Ok(media_id.to_be_bytes().to_vec())
    } else {
        Err(actix_web::error::ErrorUnauthorized("invalid session"))
    }
}

#[get("/media")]
async fn media(sessions: web::Data<ActixSessionManager>, req: HttpRequest, cache: web::Data<ActixCache>) -> Result<String, actix_web::Error> {
    let authorization_header_value = req.headers().get("AUTHORIZATION").ok_or(actix_web::error::ErrorBadRequest("missing AUTHORIZATION header"))?;
    if sessions.0.read().await.validate_session(&HashedSessionID::clone_from_slice(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(authorization_header_value).map_err(|e| {
            actix_web::error::ErrorInternalServerError(format!("failed to decode base64: {}", e))
        })?))
    {
        Ok(serde_json::to_string(cache.0.read().await.get_media())?)
    } else {
        Err(actix_web::error::ErrorUnauthorized("invalid session"))
    }
}

#[get("/media_file/{media_id}")]
async fn media_file(sessions: web::Data<ActixSessionManager>, req: HttpRequest, cache: web::Data<ActixCache>, transactions: web::Data<ActixTransactions>, media_id: web::Path<u64>) -> HttpResponse {
    let authorization_header_value = match req.headers().get("AUTHORIZATION") {
        Some(value) => value,
        None => return actix_web::error::ErrorBadRequest("missing AUTHORIZATION header").into()
    };
    let hashed_session_id_slice = match base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(authorization_header_value) {
        Ok(hashed_session_id_slice) => hashed_session_id_slice,
        Err(err) => {
            return actix_web::error::ErrorInternalServerError(format!("failed to decode base64: {}", err)).into()
        }
    };
    if sessions.0.read().await.validate_session(&HashedSessionID::clone_from_slice(&hashed_session_id_slice)) {
        let cache = cache.0.read().await;
        let cached_media = match cache.get_media().get(&media_id) {
            Some(cached_media) => cached_media,
            None => return actix_web::error::ErrorNotFound("cached media not found").into()
        };

        struct FileStream {
            offset: usize,
            size: usize,
            transactions: ReaderStream<File>,
        }

        impl futures_util::Stream for FileStream {
            type Item = std::io::Result<Bytes>;

            fn poll_next(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
                if self.offset >= self.size {
                    return Poll::Ready(None)
                }
                match Pin::new(&mut self.transactions).poll_next(cx) {
                    Poll::Pending => Poll::Pending,
                    Poll::Ready(None) => Poll::Ready(None),
                    Poll::Ready(Some(bytes_result)) => match bytes_result {
                        Ok(bytes) => {
                            if self.offset+bytes.len() > self.size {
                                self.offset = self.size;
                                Poll::Ready(Some(Ok(Bytes::copy_from_slice(&bytes[0..(self.offset+bytes.len()-self.size)]))))
                            } else {
                                self.offset += bytes.len();
                                Poll::Ready(Some(Ok(bytes)))
                            }
                        },
                        Err(err) => Poll::Ready(Some(Err(err)))
                    }
                }
            }

            fn size_hint(&self) -> (usize, Option<usize>) {
                (self.size-self.offset, Some(self.size-self.offset))
            }
        }

        let stream = match transactions.0.read().await.get_transactions_raw().await {
            Ok(mut transactions) => {
                if let Err(err) = transactions.seek(std::io::SeekFrom::Start(cached_media.file_reference.offset)).await {
                    return actix_web::error::ErrorInternalServerError(format!("failed to seek to media position: {}", err)).into()
                }
                FileStream {
                    offset: 0,
                    size: cached_media.file_reference.size as usize,
                    transactions: ReaderStream::new(transactions),
                }
                // let mut bytes = (0..cached_media.file_reference.size).map(|u| 0u8).collect::<Vec<u8>>();
                // transactions.read_exact(bytes.as_mut_slice()).await.unwrap();
                // bytes
            },
            Err(err) => return actix_web::error::ErrorInternalServerError(format!("failed to read transactions: {}", err)).into()
        };
        HttpResponse::Ok()
            .insert_header((header::CONTENT_DISPOSITION, format!("inline; filename=\"{}\"", cached_media.filename)))
            .streaming(stream)
            .into()
    } else {
        return actix_web::error::ErrorUnauthorized("invalid session").into()
    }
}

#[get("/transactions")]
async fn get_transactions(sessions: web::Data<ActixSessionManager>, req: HttpRequest, transactions: web::Data<ActixTransactions>) -> HttpResponse {
    let authorization_header_value = match req.headers().get("AUTHORIZATION") {
        Some(value) => value,
        None => return actix_web::error::ErrorBadRequest("missing AUTHORIZATION header").into()
    };
    let hashed_session_id_slice = match base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(authorization_header_value) {
        Ok(hashed_session_id_slice) => hashed_session_id_slice,
        Err(err) => {
            return actix_web::error::ErrorInternalServerError(format!("failed to decode base64: {}", err)).into()
        }
    };
    if sessions.0.read().await.validate_session(&HashedSessionID::clone_from_slice(&hashed_session_id_slice)) {
        return HttpResponse::Ok()
            .streaming(ReaderStream::new(match transactions.0.read().await.get_transactions_raw().await {
                Ok(transactions) => transactions,
                Err(err) => return actix_web::error::ErrorInternalServerError(format!("failed to get raw transacations: {}", err)).into()
            }))
            .into()
    } else {
        return actix_web::error::ErrorUnauthorized("invalid session").into()
    }
}

// #[get("/media_files_zip")]
// async fn media_files_zip(sessions: web::Data<ActixSessionManager>, req: HttpRequest, transactions: web::Data<ActixTransactions>) -> HttpResponse {
//     let authorization_header_value = match req.headers().get("AUTHORIZATION") {
//         Some(value) => value,
//         None => return actix_web::error::ErrorBadRequest("missing AUTHORIZATION header").into()
//     };
//     let hashed_session_id_slice = match base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(authorization_header_value) {
//         Ok(hashed_session_id_slice) => hashed_session_id_slice,
//         Err(err) => {
//             return actix_web::error::ErrorInternalServerError(format!("failed to decode base64: {}", err)).into()
//         }
//     };
//     if sessions.0.read().await.validate_session(&HashedSessionID::clone_from_slice(&hashed_session_id_slice)) {
//         let transactions = match transactions.0.read().await.get_transactions_raw().await {
//             Ok(transactions) => transactions,
//             Err(err) => return actix_web::error::ErrorInternalServerError(format!("failed to get raw transacations: {}", err)).into()
//         };
//         struct MediaFilesStream {
//             transactions: 
//         }

//         impl futures_util::Stream for MediaFilesStream {
//             fn poll_next(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Option<Self::Item>> {
                
//             }
//         }
//         return HttpResponse::Ok()
//             .streaming(ReaderStream::new())
//             .into()
//     } else {
//         return actix_web::error::ErrorUnauthorized("invalid session").into()
//     }
// }

#[derive(Parser)]
#[clap(author="GameSense Sports", version="v1.0.0", about="Rendering backend for Real Prep editor")]
struct Args {
    #[clap(short, long, default_value = "127.0.0.1:5050")]
    address: String,

    #[clap(short, long)]
    password: String,

    #[clap(long)]
    transactions_dir: String,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    let args = Args::parse();

    let transactions = IloveuTransactionsStore::open(args.transactions_dir).await?;

    let mut cache = IloveuCache::new();
    cache.run_raw_transactions(transactions.get_transactions_raw().await?).await?;

    let actix_transactions = ActixTransactions(Arc::new(RwLock::new(transactions)));
    let actix_cache = ActixCache(Arc::new(RwLock::new(cache)));
    let actix_sessions = ActixSessionManager(Arc::new(RwLock::new(SessionManager::new())));

    HttpServer::new(move || {
        App::new()
            .wrap(Cors::default()
                .allow_any_origin()
                .allowed_methods(["GET", "POST"])
                .allowed_headers(["AUTHORIZATION"])
            )
            .app_data(web::Data::new(Config {
                password: Arc::new(args.password.clone()),
            }))
            .app_data(web::Data::new(actix_transactions.clone()))
            .app_data(web::Data::new(actix_cache.clone()))
            .app_data(web::Data::new(actix_sessions.clone()))
            .service(login)
            .service(add_tag)
            .service(tags)
            .service(add_media)
            .service(media)
            .service(media_file)
            .service(get_transactions)
    })
        .bind(args.address)?
        .run()
        .await
}
