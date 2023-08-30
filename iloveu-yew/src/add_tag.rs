use gloo_net::http::Request;
use web_sys::{HtmlInputElement, MouseEvent};
use yew::{Html, function_component, html, Callback, TargetCast, use_state, platform::spawn_local, use_context};
use log::error;

use crate::{API_ROOT, HashedSessionIDBase64};

#[function_component]
pub fn AddTag() -> Html {
    let hashed_session_id_base64 = use_context::<HashedSessionIDBase64>().unwrap();
    let name_handle = use_state(String::default);
    let adding_handle = use_state(|| false);

    html! {
        <>
            <h1>{"Add Tag"}</h1>

            <label>{"Name: "}<input type="text" onchange={
                let name_handle = name_handle.clone();
                Callback::from(move |e: yew::Event| {
                    name_handle.set(e.target_dyn_into::<HtmlInputElement>().unwrap().value());
                })
            } value={(*name_handle).clone()}/></label><br/>

            <button onclick={
                let adding_handle = adding_handle.clone();
                Callback::from(move |_e: MouseEvent| {
                    if *adding_handle {
                        return;
                    }
                    adding_handle.set(true);
                    let name_handle = name_handle.clone();
                    let adding_handle = adding_handle.clone();
                    let hashed_session_id_base64 = hashed_session_id_base64.0.clone();
                    spawn_local(async move {
                        match Request::post(&format!("{}/add_tag", API_ROOT))
                            .header("AUTHORIZATION", &hashed_session_id_base64)
                            .body(&*name_handle)
                            .send()
                            .await {
                            Ok(response) => if response.ok() {
                                name_handle.set(String::new());
                                adding_handle.set(false);
                            } else {
                                error!("Bad response when adding tag: {:#?}", response.text().await);
                                adding_handle.set(false);
                            },
                            Err(err) => {
                                error!("Failed to send add tag request: {}", err);
                                adding_handle.set(false);
                            }
                        }
                    })
                })
            } disabled={*adding_handle}>{if *adding_handle {"Adding"} else {"Add"}}</button>
        </>
    }
}