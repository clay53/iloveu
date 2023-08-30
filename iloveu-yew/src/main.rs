use base64::Engine;
use gloo_net::http::Request;
use gloo_storage::{SessionStorage, Storage};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{HtmlInputElement, Blob, Url, window, HtmlElement};
use yew::{function_component, Html, html, use_state, MouseEvent, Callback, TargetCast, platform::spawn_local, Properties, ContextProvider, classes};
use yew_router::{Routable, BrowserRouter, Switch, prelude::Link};
use iloveu_yew::{API_ROOT, HashedSessionIDBase64, home::Home, add_media::AddMedia, add_tag::AddTag};

use log::{info, error, warn};

#[derive(Debug, Clone, Routable, PartialEq)]
enum Route {
    #[at("/")]
    Home,
    #[at("/add_tag")]
    AddTag,
    #[at("/add_media")]
    AddMedia,
    #[not_found]
    #[at("/404")]
    NotFound,
}

#[function_component]
fn App() -> Html {
    let hashed_session_id_base64_handle = use_state(|| {
        match SessionStorage::get::<String>("hashed_session_id_base64") {
            Ok(hashed_session_id_base64) => {
                Option::<String>::Some(hashed_session_id_base64)
            },
            Err(err) => {
                warn!("Failed to get hashed_session_id_base64 from storage: {}", err);
                Option::<String>::None
            }
        }
    });

    html! {
        if (*hashed_session_id_base64_handle).is_some() {
            <ContextProvider<HashedSessionIDBase64> context={HashedSessionIDBase64((*hashed_session_id_base64_handle).clone().unwrap())}>
                <BrowserRouter>
                    <Switch<Route> render={switch}/>
                </BrowserRouter>
            </ContextProvider<HashedSessionIDBase64>>
            <button onclick={
                let hashed_session_id_handle = hashed_session_id_base64_handle.clone();
                Callback::from(move |_e: MouseEvent| {
                    SessionStorage::delete("hashed_session_id");
                    hashed_session_id_handle.set(None);
                })
            }>{"Sign out"}</button>
            <button onclick={
                let hashed_session_id_handle = hashed_session_id_base64_handle.clone();
                Callback::from(move |_e: MouseEvent| {
                    let hashed_session_id_handle = hashed_session_id_handle.clone();
                    spawn_local(async move {
                        match Request::get(&format!("{}/transactions", API_ROOT))
                            .header("AUTHORIZATION", hashed_session_id_handle.as_ref().unwrap())
                            .send()
                            .await {
                            Ok(response) => if response.ok() {
                                match response.as_raw().blob() {
                                    Ok(blob_promise) => match JsFuture::from(blob_promise).await {
                                        Ok(blob) => {
                                            let blob = blob.dyn_into::<Blob>().unwrap();
                                            let url = Url::create_object_url_with_blob(&blob).unwrap();
                                            let document = window().unwrap().document().unwrap();
                                            let body = document.body().unwrap();
                                            let a = document.create_element("a").unwrap();
                                            a.set_attribute("href", &url).unwrap();
                                            a.set_attribute("download", "transactions").unwrap();
                                            body.append_child(&a).unwrap();
                                            a.dyn_into::<HtmlElement>().unwrap().click();
                                        },
                                        Err(err) => error!("Failed to get transactions blob from blob promise: {:#?}", err)
                                    },
                                    Err(err) => error!("Failed to get transactions blob promise: {:#?}", err)
                                }
                            } else {
                                error!("Bad response when downloading transactions: {:#?}", response.text().await);
                            },
                            Err(err) => error!("Failed to send download transaction request: {}", err)
                        }
                    });
                })
            }>{"Download Transactions"}</button>
            // <button onclick={
            //     Callback::from(move |_e: MouseEvent| {
            //         let hashed_session_id_handle = hashed_session_id_base64_handle.clone();
            //         spawn_local(async move {
            //             match Request::get(&format!("{}/media_files_zip", API_ROOT))
            //                 .header("AUTHORIZATION", hashed_session_id_handle.as_ref().unwrap())
            //                 .send()
            //                 .await {
            //                 Ok(response) => if response.ok() {
            //                     match response.as_raw().blob() {
            //                         Ok(blob_promise) => match JsFuture::from(blob_promise).await {
            //                             Ok(blob) => {
            //                                 let blob = blob.dyn_into::<Blob>().unwrap();
            //                                 let url = Url::create_object_url_with_blob(&blob).unwrap();
            //                                 let document = window().unwrap().document().unwrap();
            //                                 let body = document.body().unwrap();
            //                                 let a = document.create_element("a").unwrap();
            //                                 a.set_attribute("href", &url).unwrap();
            //                                 a.set_attribute("download", "media-files.zip").unwrap();
            //                                 body.append_child(&a).unwrap();
            //                                 a.dyn_into::<HtmlElement>().unwrap().click();
            //                             },
            //                             Err(err) => error!("Failed to get transactions blob from blob promise: {:#?}", err)
            //                         },
            //                         Err(err) => error!("Failed to get transactions blob promise: {:#?}", err)
            //                     }
            //                 } else {
            //                     error!("Bad response when downloading transactions: {:#?}", response.text().await);
            //                 },
            //                 Err(err) => error!("Failed to send download transaction request: {}", err)
            //             }
            //         });
            //     })
            // }>{"Download Media Files"}</button>
        } else {
            <Login on_set_hashed_session_id={
                Callback::from(move |e: [u8; 32]| {
                    let hashed_session_id_base64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(e);
                    match SessionStorage::set("hashed_session_id_base64", hashed_session_id_base64.clone()) {
                        Ok(()) => {}
                        Err(err) => error!("Failed to store hashed_session_id_base64: {}", err),
                    };
                    hashed_session_id_base64_handle.set(Some(hashed_session_id_base64));
                })
            }/>
        }
    }
}

#[derive(Debug, Properties, PartialEq)]
struct NavBarProps {
    route: Route,
}

#[function_component]
fn NavBar(props: &NavBarProps) -> Html {
    html! {
        <nav>
        <div class={classes!(if props.route == Route::Home {vec!["nav-item-selected"]} else {vec![]})}><Link<Route> to={Route::Home}>{"Home"}</Link<Route>></div>
        <div class={classes!(if props.route == Route::AddTag {vec!["nav-item-selected"]} else {vec![]})}><Link<Route> to={Route::AddTag}>{"Add Tag"}</Link<Route>></div>
        <div class={classes!(if props.route == Route::AddMedia {vec!["nav-item-selected"]} else {vec![]})}><Link<Route> to={Route::AddMedia}>{"Add Media"}</Link<Route>></div>
        </nav>
    }
}

fn switch(route: Route) -> Html {
    html! {
        <>
            <NavBar route={route.clone()}/>
            <div id="content">
                {match route {
                    Route::Home => html! {<Home/>},
                    Route::AddTag => html! {<AddTag/>},
                    Route::AddMedia => html! {<AddMedia/>},
                    Route::NotFound => html! { <Link<Route> to={Route::Home}>{"Page not found, return home"}</Link<Route>> }
                }}
            </div>
        </>
    }
}

#[derive(Properties, PartialEq)]

struct LoginProps {
    pub on_set_hashed_session_id: Callback<[u8; 32]>,
}

#[function_component]
fn Login(props: &LoginProps) -> Html {
    let password_handle = use_state(String::default);

    html! {
        <>
            <h1>{"Login"}</h1>
            <label>{"password: "}<input type="password" onchange={
                let password_handle = password_handle.clone();
                Callback::from(move |e: yew::Event| {
                    password_handle.set(e.target_dyn_into::<HtmlInputElement>().unwrap().value());
                })
            } value={(*password_handle).clone()}/></label>
            <br/>
            <button onclick={
                let on_set_hashed_session_id = props.on_set_hashed_session_id.clone();
                Callback::from(move |_e: MouseEvent| {
                    let on_set_hashed_session_id = on_set_hashed_session_id.clone();
                    let password_handle = password_handle.clone();
                    spawn_local(async move {
                        let login_response = Request::post(format!("{}/login", API_ROOT).as_str())
                            .body(&*password_handle)
                            .send()
                            .await;
                        match login_response {
                            Ok(response) => {
                                if response.ok() {
                                    match response.binary().await {
                                        Ok(hashed_session_id_vec) => {
                                            info!("Got hashed session id");
                                            let mut hashed_session_id: [u8; 32] = [0; 32];
                                            for (i, byte) in hashed_session_id_vec.iter().enumerate() {
                                                if i < 32 {
                                                    hashed_session_id[i] = *byte;
                                                } else {
                                                    break;
                                                }
                                            }
                                            on_set_hashed_session_id.emit(hashed_session_id);
                                        },
                                        Err(err) => error!("Failed to get binary of login response: {}", err),
                                    }
                                } else {
                                    error!("Bad login response: {:#?}", response.text().await);
                                }
                            },
                            Err(err) => error!("Failed to send login request: {:#?}", err)
                        }
                    });
                })
            }>{"Login"}</button>
        </>
    }
}

fn main() {
    wasm_logger::init(wasm_logger::Config::default());
    yew::Renderer::<App>::new().render();
}
