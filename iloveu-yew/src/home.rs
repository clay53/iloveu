use std::{collections::HashMap, str::FromStr};

use gloo_net::http::Request;
use js_sys::{Map, JsString, Date};
use log::error;
use serde::Deserialize;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{window, RequestInit, Blob, Url};
use yew::{Html, function_component, html, use_state, use_effect_with_deps, platform::spawn_local, use_context};

use crate::{API_ROOT, HashedSessionIDBase64};

#[function_component]
pub fn Home() -> Html {
    let hashed_session_id_base64 = use_context::<HashedSessionIDBase64>().unwrap();

    let media_handle = use_state(HashMap::<u64, MediaInfo>::default);
    let media_files_handle = use_state(HashMap::<u64, String>::default);
    
    let media_handle_effect = media_handle.clone();
    let hashed_session_id_base64_effect = hashed_session_id_base64.clone();
    use_effect_with_deps(move |_| {
        let media_handle_effect = media_handle_effect.clone();
        spawn_local(async move {
            match Request::get(&format!("{}/media", API_ROOT))
                .header("AUTHORIZATION", &hashed_session_id_base64_effect.0)
                .send()
                .await {
                Ok(response) => if response.ok() {
                    match response.json::<HashMap<u64, MediaInfo>>().await {
                        Ok(media) => {
                            media_handle_effect.set(media);
                        },
                        Err(err) => error!("Failed to parse JSON from tags response: {}", err)
                    }
                } else {
                    error!("Bad response when getting tags: {:#?}", response.text().await);
                },
                Err(err) => error!("Failed to send response for tags: {}", err)
            }
        });
    }, ());

    let sorted_media_handle = use_state(Vec::<(u64, MediaInfo)>::default);
    let sorted_media_handle_effect = sorted_media_handle.clone();
    use_effect_with_deps(move |media_handle| {
        let mut media: Vec<(u64, MediaInfo)> = (**media_handle).clone().into_iter().collect();
        media.sort_by(|a, b| b.1.taken_datetime.total_cmp(&a.1.taken_datetime));
        sorted_media_handle_effect.set(media);
    }, media_handle.clone());

    let media_files_handle_effect = media_files_handle.clone();
    let hashed_session_id_base64_effect = hashed_session_id_base64.clone();
    use_effect_with_deps(move |media_handle| {
        let media_handle = media_handle.clone();
        let media_files_handle = media_files_handle_effect.clone();
        let hashed_session_id_base64 = hashed_session_id_base64_effect.clone();
        spawn_local(async move {
            let mut media_files = HashMap::new();
            for (media_id, _) in media_handle.iter() {
                match JsFuture::from(window().unwrap().fetch_with_str_and_init(
                    &format!("{}/media_file/{}", API_ROOT, media_id),
                    &RequestInit::new()
                        .method("get")
                        .headers(&Map::new().set(&JsString::from_str("AUTHORIZATION").unwrap(), &JsString::from_str(&(*hashed_session_id_base64.0)).unwrap()))
                )).await {
                    Ok(response) => {
                        let response = response.dyn_into::<web_sys::Response>().unwrap();
                        if response.ok() {
                            let blob = match response.blob() {
                                Ok(blob_promise) => match JsFuture::from(blob_promise).await {
                                    Ok(blob) => blob.dyn_into::<Blob>().unwrap(),
                                    Err(err) => {
                                        error!("Failed to get blob from media file blob promise: {:#?}", err);
                                        return;
                                    }
                                },
                                Err(err) => {
                                    error!("Failed to get blob promise from media file response: {:#?}", err);
                                    return;
                                }
                            };
                            let data_url = Url::create_object_url_with_blob(&blob).unwrap();
                            media_files.insert(*media_id, data_url);
                        } else {
                            error!("Bad response from media file request: {:#?}", response.text());
                        }
                    },
                    Err(err) => error!("Failed to send media file request: {:#?}", err)
                }
            }
            media_files_handle.set(media_files);
        });
    }, media_handle.clone());

    let tags_handle = use_state(HashMap::<u64, String>::default);

    let tags_handle_effect = tags_handle.clone();
    let hashed_session_id_base64_for_tags = hashed_session_id_base64.clone();
    use_effect_with_deps(move |_| {
        let tags_handle = tags_handle_effect;
        spawn_local(async move {
            match Request::get(&format!("{}/tags", API_ROOT))
                .header("AUTHORIZATION", &hashed_session_id_base64_for_tags.0)
                .send()
                .await {
                Ok(response) => if response.ok() {
                    match response.json::<HashMap<u64, String>>().await {
                        Ok(tags) => {
                            tags_handle.set(tags);
                        },
                        Err(err) => error!("Failed to parse JSON from tags response: {}", err)
                    }
                } else {
                    error!("Bad response when getting tags: {:#?}", response.text().await);
                },
                Err(err) => error!("Failed to send response for tags: {}", err)
            }
        })
    }, ());

    html! {
        <div class="media-grid">
            {(*sorted_media_handle).iter().map(|(media_id, media)| html! {
                <div key={*media_id} class="media">
                    <h2>{&media.title}</h2>
                    {match media_files_handle.get(media_id) {
                        Some(src) => match media.media_type {
                            MediaType::Picture => html! {
                                <img class="media-img" src={src.clone()}/>
                            },
                            MediaType::Video => html! {
                                <video class="media-video" src={src.clone()} controls=true/>
                            }
                        },
                        None => html! {}
                    }}
                    <p>
                        if media.tags_vec.len() > 0 {
                            {format!("tags: {}", media.tags_vec.iter().map(|tag_id| {
                                (*tags_handle).get(tag_id).unwrap_or(&format!("UNKNOWN({})", tag_id)).clone()
                            }).intersperse_with(|| ", ".to_string()).collect::<String>())}<br/>
                        }
                        {format!("Date: {}", (|| {
                            let date = Date::new_0();
                            date.set_time(media.taken_datetime);
                            date.to_string()
                        })())}<br/>
                        {&media.description}
                    </p>
                </div>
            }).collect::<Html>()}
        </div>
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
struct MediaInfo {
    pub title: String,
    pub description: String,
    pub tags_vec: Vec<u64>,
    pub taken_datetime: f64,
    pub media_type: MediaType,
    pub filename: String,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
enum MediaType {
    Picture,
    Video,
}