use rust_transitive_reach_same_name_other_type_fixture::{Lang, Site};

pub fn build_site(langs: &[(&str, bool)]) -> Vec<String> {
    let site = Site {
        langs: langs
            .iter()
            .map(|(code, feeds)| Lang {
                code: code.to_string(),
                feeds: *feeds,
            })
            .collect(),
    };
    site.build()
}
