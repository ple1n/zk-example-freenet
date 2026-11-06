use std::{cell::RefCell, rc::Rc};

use dioxus::prelude::*;
use freenet_stdlib::{
    client_api::{
        ClientError, ClientRequest, ContractRequest, ContractResponse, Error as WebApiError,
        HostResponse, WebApi,
    },
    prelude::{ContractInstanceId, StateDelta, UpdateData},
};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{HtmlInputElement, WebSocket};

fn default_endpoint() -> String {
    "ws://127.0.0.1:7509/v2/contract/command?encodingProtocol=native".into()
}

fn submit(
    endpoint: String,
    instance_id: ContractInstanceId,
    candidate: Vec<u8>,
    status: Signal<String>,
    events: Signal<Vec<String>>,
) -> Result<(), String> {
    let socket = WebSocket::new(&endpoint)
        .map_err(|error| format!("Could not open WebSocket: {error:?}"))?;

    let api = Rc::new(RefCell::new(None::<WebApi>));
    let pending = Rc::new(RefCell::new(Some((instance_id, candidate))));

    let api_for_response = Rc::clone(&api);
    let pending_for_response = Rc::clone(&pending);
    let mut result_status = status;
    let mut result_events = events;
    let result_handler = move |result: Result<HostResponse, ClientError>| match result {
        Ok(HostResponse::ContractResponse(ContractResponse::GetResponse {
            key,
            contract: Some(contract),
            state,
        })) => {
            let Some((expected_id, candidate)) = pending_for_response.borrow_mut().take() else {
                return;
            };
            if key.id() != &expected_id {
                *pending_for_response.borrow_mut() = Some((expected_id, candidate));
                result_events.write().push(format!(
                    "Ignored GetResponse for a different contract: key={key}"
                ));
                return;
            }
            result_events.write().push(format!(
                "Received ContractResponse::GetResponse: key={key}, contract_code_returned=true, current_state_bytes={}",
                state.as_ref().len()
            ));
            let request = ClientRequest::ContractOp(ContractRequest::Update {
                key: contract.key(),
                data: UpdateData::Delta(StateDelta::from(candidate)),
            });
            let api = Rc::clone(&api_for_response);
            let mut result_status = result_status;
            let mut result_events = result_events;
            spawn_local(async move {
                let result = {
                    let mut api = api.borrow_mut();
                    match api.as_mut() {
                        Some(api) => api.send(request).await.map_err(|error| error.to_string()),
                        None => Err("The node connection closed before submission.".into()),
                    }
                };
                match result {
                    Ok(()) => {
                        result_status.set(
                            "Update request sent. Waiting for the node response; this does not mean the contract accepted the candidate.".into(),
                        );
                        result_events
                            .write()
                            .push("Sent ClientRequest::ContractOp(Update)".into());
                    }
                    Err(error) => {
                        result_status.set(format!("Update send failed: {error}"));
                        result_events
                            .write()
                            .push(format!("WebApi::send(Update) failed: {error}"));
                    }
                }
            });
        }
        Ok(HostResponse::ContractResponse(ContractResponse::GetResponse {
            key,
            contract: None,
            state,
        })) => {
            result_status.set(
                "The GetResponse did not include contract code; cannot construct an Update key."
                    .into(),
            );
            result_events.write().push(format!(
                "Received ContractResponse::GetResponse: key={key}, contract_code_returned=false, current_state_bytes={}",
                state.as_ref().len()
            ));
        }
        Ok(HostResponse::ContractResponse(ContractResponse::UpdateResponse { key, summary })) => {
            result_status.set("Received UpdateResponse from the node.".into());
            result_events.write().push(format!(
                "Received ContractResponse::UpdateResponse: key={key}, summary_bytes={}",
                summary.as_ref().len()
            ));
        }
        Ok(HostResponse::ContractResponse(ContractResponse::NotFound { instance_id })) => {
            result_status.set("The node reported that the contract was not found.".into());
            result_events.write().push(format!(
                "Received ContractResponse::NotFound: id={instance_id}"
            ));
        }
        Ok(other) => {
            result_events
                .write()
                .push(format!("Received other host response: {other:?}"));
        }
        Err(error) => {
            result_status.set(format!("Node returned an error: {error}"));
            result_events
                .write()
                .push(format!("Host response error: {error:?}"));
        }
    };

    let api_for_open = Rc::clone(&api);
    let mut open_status = status;
    let mut error_status = status;
    let mut open_events = events;
    let mut error_events = events;
    let instance_for_open = instance_id;
    let web_api = WebApi::start(
        socket,
        result_handler,
        move |error: WebApiError| {
            error_status.set(format!("WebSocket/API error: {error}"));
            error_events
                .write()
                .push(format!("WebApi error: {error:?}"));
        },
        move || {
            open_status
                .set("Connected. Sending a Get request to retrieve the contract key...".into());
            open_events.write().push("WebSocket opened".into());
            let api = Rc::clone(&api_for_open);
            spawn_local(async move {
                let request = ClientRequest::ContractOp(ContractRequest::Get {
                    key: instance_for_open,
                    return_contract_code: true,
                    subscribe: false,
                    blocking_subscribe: false,
                });
                let result = {
                    let mut api = api.borrow_mut();
                    match api.as_mut() {
                        Some(api) => api.send(request).await.map_err(|error| error.to_string()),
                        None => Err("The node connection closed before lookup.".into()),
                    }
                };
                match result {
                    Ok(()) => {
                        open_status
                            .set("Get request sent. Waiting for the contract response...".into());
                        open_events.write().push(
                            "Sent ClientRequest::ContractOp(Get { return_contract_code: true })"
                                .into(),
                        );
                    }
                    Err(error) => {
                        open_status.set(format!("Get send failed: {error}"));
                        open_events
                            .write()
                            .push(format!("WebApi::send(Get) failed: {error}"));
                    }
                }
            });
        },
    );
    *api.borrow_mut() = Some(web_api);
    Ok(())
}

#[component]
fn App() -> Element {
    let mut contract_id = use_signal(String::new);
    let mut endpoint = use_signal(default_endpoint);
    let candidate = use_signal(|| None::<Vec<u8>>);
    let candidate_name = use_signal(String::new);
    let mut status = use_signal(|| "Enter a contract ID and upload its candidate file.".to_owned());
    let mut events = use_signal(Vec::<String>::new);

    let read_candidate = move |_| {
        let Some(input) = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.get_element_by_id("candidate-file"))
            .and_then(|element| element.dyn_into::<HtmlInputElement>().ok())
        else {
            status.set("Could not access the candidate file input.".into());
            return;
        };
        let Some(file) = input.files().and_then(|files| files.get(0)) else {
            return;
        };
        let name = file.name();
        let mut status = status;
        let mut candidate = candidate;
        let mut candidate_name = candidate_name;
        spawn_local(async move {
            let result = JsFuture::from(file.text())
                .await
                .map_err(|error| format!("Could not read the file: {error:?}"))
                .and_then(|value| {
                    value
                        .as_string()
                        .ok_or_else(|| "The selected file did not contain text.".to_owned())
                });
            match result {
                Ok(text) => {
                    candidate.set(Some(text.into_bytes()));
                    candidate_name.set(name);
                    status.set(
                        "File loaded without inspection. Ready to submit its bytes unchanged."
                            .into(),
                    );
                }
                Err(error) => {
                    candidate.set(None);
                    candidate_name.set(String::new());
                    status.set(format!("Invalid candidate: {error}"));
                }
            }
        });
    };

    let submit_candidate = move |_| {
        let instance_id = match ContractInstanceId::from_base58(contract_id().trim()) {
            Ok(id) => id,
            Err(error) => {
                status.set(format!("Invalid contract ID: {error}"));
                return;
            }
        };
        let Some(candidate) = candidate() else {
            status.set("Upload a valid candidate JSON file first.".into());
            return;
        };
        status.set("Connecting to the local Freenet node...".into());
        events.set(Vec::new());
        if let Err(error) = submit(endpoint(), instance_id, candidate, status, events) {
            status.set(error);
        }
    };

    rsx! {
        style { {include_str!("../assets/style.css")} }
        main {
            section { class: "card",
                p { class: "eyebrow", "SP1 • FRENET CONTRACT TESTER" }
                h1 { "Submit a proof candidate" }
                p { class: "intro",
                    "A developer demo for sending candidate file bytes to an already-deployed SP1 contract. Proof generation and contract deployment remain CLI steps. The browser does not inspect or verify the candidate."
                }
                label { r#for: "contract-id", "Local contract ID" }
                input {
                    id: "contract-id",
                    value: "{contract_id}",
                    placeholder: "Paste the ID printed by fdev local execute put",
                    oninput: move |event| contract_id.set(event.value())
                }
                label { r#for: "node-endpoint", "Node WebSocket endpoint" }
                input {
                    id: "node-endpoint",
                    value: "{endpoint}",
                    oninput: move |event| endpoint.set(event.value())
                }
                label { r#for: "candidate-file", "Candidate JSON" }
                input {
                    id: "candidate-file",
                    r#type: "file",
                    onchange: read_candidate
                }
                if !candidate_name().is_empty() {
                    p { class: "filename", "Loaded: {candidate_name}" }
                }
                button {
                    class: "submit",
                    disabled: candidate().is_none(),
                    onclick: submit_candidate,
                    "Submit candidate"
                }
                p { class: "status", role: "status", "{status}" }
                p { class: "hint",
                    "The UI sends a Get request to retrieve the contract code/key, then sends the uploaded bytes as an Update delta. It reports WebSocket/API responses separately from sending the request."
                }
                if !events.read().is_empty() {
                    section { class: "responses",
                        h2 { "WebSocket / node responses" }
                        for event in events.read().iter() {
                            pre { "{event}" }
                        }
                    }
                }
            }
        }
    }
}

fn main() {
    dioxus::launch(App);
}
