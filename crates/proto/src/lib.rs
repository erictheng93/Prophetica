pub mod proph {
    pub mod v1 {
        #![allow(
            clippy::doc_markdown,
            clippy::default_trait_access,
            clippy::missing_errors_doc,
            clippy::must_use_candidate,
            clippy::similar_names,
            clippy::too_many_lines,
            clippy::used_underscore_items,
            clippy::unwrap_used,
            clippy::wildcard_imports
        )]

        include!(concat!(env!("OUT_DIR"), "/proph.v1.rs"));
    }
}
