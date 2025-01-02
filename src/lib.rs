pub mod modules {
    pub mod event_handler;
    pub mod networking;
    pub mod peer_list;
    pub mod peer_state;
    pub mod protocol;
    pub mod tui;
    pub mod widgets {
        pub mod message_bubble;
        pub mod peer_bubble;
        pub mod list_component;
    }
}

pub mod config;
