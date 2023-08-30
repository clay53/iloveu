use gloo_net::http::Request;
use js_sys::{Map, JsString, Date};
use log::{error, info};
use web_sys::{HtmlInputElement, MouseEvent, HtmlTextAreaElement, HtmlSelectElement, File, Url, HtmlOptionElement, window, RequestInit, FormData};
use yew::{Html, function_component, html, use_state, Callback, TargetCast, use_memo, use_effect_with_deps, platform::spawn_local, use_context};
use std::{collections::HashMap, str::FromStr};
use wasm_bindgen::JsCast;

use wasm_bindgen_futures::JsFuture;

use crate::{API_ROOT, HashedSessionIDBase64};

#[function_component]
pub fn AddMedia() -> Html {
    let title_handle = use_state(String::default);
    let description_handle = use_state(String::default);
    let type_handle = use_state(|| String::from("picture"));
    let file_handle = use_state(Option::<File>::default);
    let taken_datetime_handle = use_state(String::default);
    let file_url_handle = use_memo(|file_handle| {
        match &**file_handle {
            Some(file) => Url::create_object_url_with_blob(&*file).ok(),
            None => None
        }
    }, file_handle.clone());
    let hashed_session_id_base64 = use_context::<HashedSessionIDBase64>().unwrap();
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

    let chosen_tags = use_state(Vec::<u64>::default);
    
    let adding_handle = use_state(|| false);

    html! {
        <>
            <h1>{"Add Media"}</h1>

            <label>{"Title: "}<input type="text" onchange={
                let title_handle = title_handle.clone();
                Callback::from(move |e: yew::Event| {
                    title_handle.set(e.target_dyn_into::<HtmlInputElement>().unwrap().value());
                })
            }/></label><br/>

            <label>{"Description: "}<textarea onchange={
                let description_handle = description_handle.clone();
                Callback::from(move |e: yew::Event| {
                    description_handle.set(e.target_dyn_into::<HtmlTextAreaElement>().unwrap().value());
                })
            }/></label><br/>

            <label>{"Tags: "}<select onchange={
                let chosen_tags = chosen_tags.clone();
                Callback::from(move |e: yew::Event| {
                    let mut tag_ids: Vec<u64> = Vec::new();
                    let selected_options = e.target_dyn_into::<HtmlSelectElement>().unwrap().selected_options();
                    for option_index in 0..selected_options.length() {
                        if let Some(option) = selected_options.item(option_index) {
                            match option.dyn_ref::<HtmlOptionElement>().unwrap().value().parse::<u64>() {
                                Ok(tag_id) => tag_ids.push(tag_id),
                                Err(err) => error!("Failed to parse option value as tag id: {}", err)
                            }
                        }
                    }
                    
                    chosen_tags.set(tag_ids);
                })
            } multiple=true>
                {(*tags_handle).iter().map(|tag| {
                    html! { <option key={*tag.0} value={format!("{}",tag.0)}>{tag.1}</option> }
                }).collect::<Html>()}
            </select></label><br/>

            <label>{"Taken date & time: "}<input type="datetime-local" onchange={
                let taken_datetime_handle = taken_datetime_handle.clone();
                Callback::from(move |e: yew::Event| {
                    taken_datetime_handle.set(e.target_dyn_into::<HtmlInputElement>().unwrap().value());
                })
            }/></label><br/>

            <label>{"Media type: "}<select onchange={
                let type_handle = type_handle.clone();
                Callback::from(move |e: yew::Event| {
                    type_handle.set(e.target_dyn_into::<HtmlSelectElement>().unwrap().value());
                })
            }>
                <option value="video">{"Video"}</option>
                <option value="picture">{"Picture"}</option>
            </select></label><br/>

            <label>{"File: "}<input type="file" onchange={
                let file_handle = file_handle.clone();
                Callback::from(move |e: yew::Event| {
                    file_handle.set(e.target_dyn_into::<HtmlInputElement>().unwrap().files().unwrap().get(0));
                })
            }/></label><br/>

            {match &*file_url_handle {
                Some(url) => html! {
                    {match (*type_handle).as_str() {
                        "video" => html! {
                            <><video style="max-width: 80%; max-height: 80vh;" src={url.clone()} controls=true/><br/></>
                        },
                        "picture" => html! {
                            <><img style="max-width: 80%; max-height: 80vh;" src={url.clone()}/><br/></>
                        },
                        _ => html! {}
                    }}
                },
                None => html! {}
            }}

            <button onclick={
                let adding_handle = adding_handle.clone();
                Callback::from(move |_e: MouseEvent| {
                    if *adding_handle {
                        return;
                    }
                    adding_handle.set(true);
                    let title_handle = title_handle.clone();
                    let description_handle = description_handle.clone();
                    let chosen_tags = chosen_tags.clone();
                    let type_handle = type_handle.clone();
                    let file_handle = file_handle.clone();
                    let taken_datetime_handle = taken_datetime_handle.clone();
                    let hashed_session_id_base64 = hashed_session_id_base64.clone();
                    let adding_handle = adding_handle.clone();
                    spawn_local(async move {
                        let body = FormData::new().unwrap();
                        body.append_with_str("title", &*title_handle).unwrap();
                        body.append_with_str("description", &*description_handle).unwrap();
                        body.append_with_str("tags", &serde_json::to_string(&*chosen_tags).unwrap()).unwrap();
                        let taken_datetime_local = Date::parse(&*taken_datetime_handle);
                        let timezone_offset = Date::new_0().get_timezone_offset();
                        let taken_datetime = taken_datetime_local+timezone_offset;
                        body.append_with_str("taken_datetime", &serde_json::to_string(&taken_datetime).unwrap()).unwrap();
                        body.append_with_str("media_type", &*type_handle).unwrap();
                        let file = (*file_handle).as_ref().unwrap();
                        body.append_with_blob_and_filename("file", &file, &file.name()).unwrap();
                        match JsFuture::from(window().unwrap().fetch_with_str_and_init(
                            &format!("{}/add_media", API_ROOT),
                            &RequestInit::new()
                                .method("post")
                                .body(Some(&body))
                                .headers(&Map::new().set(&JsString::from_str("AUTHORIZATION").unwrap(), &JsString::from_str(&(*hashed_session_id_base64.0)).unwrap()))
                        )).await {
                            Ok(response) => {
                                let response = gloo_net::http::Response::from_raw(response.dyn_into::<web_sys::Response>().unwrap());
                                if response.ok() {
                                    info!("Successfully added media");
                                } else {
                                    error!("Bad response from add media request: {:#?}", response.text().await);
                                }
                            },
                            Err(err) => error!("Failed to send add media request: {:#?}", err)
                        };
                        adding_handle.set(false);
                    })
                })
            } disabled={(*file_handle).is_none() || (*taken_datetime_handle).len() == 0 || (*adding_handle)}>{if *adding_handle {"Adding"} else {"Add"}}</button>
        </>
    }
}