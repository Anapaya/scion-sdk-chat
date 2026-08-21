// Copyright 2026 Anapaya Systems
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//! The three screens, as components over [`State`].

use dioxus::prelude::*;

use crate::app::{State, Submission};

/// Where the server is.
#[component]
pub fn Connection() -> Element {
    let state = use_context::<State>();
    let mut server_url = state.server_url;
    let busy = (state.busy)();

    rsx! {
        div { class: "centred",
            form {
                class: "form",
                // Enter submits, and the browser's own reload on submit is what has to be stopped.
                onsubmit: move |event| {
                    event.prevent_default();
                    spawn(state.connect());
                },
                h1 { "Connect" }
                input {
                    class: "field",
                    value: "{state.server_url}",
                    oninput: move |event| server_url.set(event.value()),
                    disabled: busy,
                }
                button { class: "primary", r#type: "submit", disabled: busy,
                    if busy { "Connecting…" } else { "Connect" }
                }
                Error {}
            }
        }
    }
}

/// Who is signing in.
#[component]
pub fn SignIn() -> Element {
    let state = use_context::<State>();
    let (mut username, mut password) = (state.username, state.password);
    let busy = (state.busy)();
    let ready = !busy && !state.username.read().is_empty() && !state.password.read().is_empty();

    rsx! {
        div { class: "centred",
            form {
                class: "form",
                onsubmit: move |event| {
                    event.prevent_default();
                    spawn(state.log_in());
                },
                h1 { "Sign in" }
                input {
                    class: "field",
                    placeholder: "username",
                    value: "{state.username}",
                    oninput: move |event| username.set(event.value()),
                    disabled: busy,
                }
                input {
                    class: "field",
                    r#type: "password",
                    placeholder: "password",
                    value: "{state.password}",
                    oninput: move |event| password.set(event.value()),
                    disabled: busy,
                }
                div { class: "row",
                    button {
                        class: "secondary",
                        r#type: "button",
                        disabled: !ready,
                        onclick: move |_| { spawn(state.register()); },
                        "Register"
                    }
                    button { class: "primary", r#type: "submit", disabled: !ready, "Log in" }
                }
                Error {}
            }
        }
    }
}

/// The rooms, the open room's messages, and what is being typed.
#[component]
pub fn Chat() -> Element {
    let state = use_context::<State>();

    rsx! {
        div { class: "chat",
            Sidebar {}
            div { class: "pane",
                header { class: "header", "#{state.open_name()}" }
                Messages {}
                Composer {}
            }
        }
    }
}

#[component]
fn Sidebar() -> Element {
    let mut state = use_context::<State>();
    let open = (state.open)();

    rsx! {
        nav { class: "sidebar",
            div { class: "sidebar-title", "Rooms" }
            for room in state.rooms.read().iter().cloned() {
                button {
                    key: "{room.id.get()}",
                    // A dot rather than a number, for the reason `State::unread` gives.
                    class: if Some(room.id) == open { "room open" } else if state.unread(&room) { "room unread" } else { "room" },
                    onclick: move |_| state.open_room(room.id),
                    if state.unread(&room) { "#{room.name} ●" } else { "#{room.name}" }
                }
            }
        }
    }
}

#[component]
fn Messages() -> Element {
    let state = use_context::<State>();
    let me = state.username.read().clone();

    rsx! {
        // `column-reverse` puts the first child at the bottom and starts scrolled there, which is
        // what keeps the newest line in view without measuring anything. Newest first, therefore.
        div { class: "messages",
            for message in state.messages.read().iter().rev() {
                div { key: "{message.seq.get()}", class: "line",
                    // Their own name, so a glance finds it.
                    span {
                        class: if message.username == me { "sender me" } else { "sender" },
                        "{message.username}"
                    }
                    span { class: "body", "{message.body}" }
                }
            }
        }
    }
}

#[component]
fn Composer() -> Element {
    let state = use_context::<State>();
    let mut draft = state.draft;
    // Naming the action is what tells the reader that Enter is about to do something other than
    // post what they typed.
    let (label, ready) = match state.submission() {
        Some(Submission::Create(_)) => ("Create", true),
        Some(Submission::Send(_)) => ("Send", true),
        None => ("Send", false),
    };

    rsx! {
        form {
            class: "composer",
            onsubmit: move |event| {
                event.prevent_default();
                spawn(state.submit());
            },
            input {
                class: "field",
                placeholder: "type a message, or /room name to create one",
                value: "{state.draft}",
                oninput: move |event| draft.set(event.value()),
            }
            button { class: "primary", r#type: "submit", disabled: !ready, "{label}" }
            Error {}
        }
    }
}

/// The error line, shown until something succeeds.
#[component]
fn Error() -> Element {
    let state = use_context::<State>();

    rsx! {
        match state.error.read().as_deref() {
            Some(error) => rsx! { div { class: "error", "⚠ {error}" } },
            // A held space rather than nothing, so an error does not move the form.
            None => rsx! { div { class: "error-space" } },
        }
    }
}
