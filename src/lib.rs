use codeowners::Owners;

pub mod cache;
pub mod conditional;
pub mod config;
pub mod github;
pub mod metrics;

pub fn display_file_owners(codeowners: &Owners, files: &[&str]) -> String {
    let mut display_str = "<ul>".to_string();
    let owners_map = conditional::to_owners_map(codeowners, files);
    for (file, owners) in owners_map.into_iter() {
        display_str.push_str(&format!("<li><code>{}</code>", file));
        if let Some(owners) = owners {
            let owner_lines = owners
                .iter()
                .map(|owner| format!("<li>{owner}</li>"))
                .collect::<Vec<_>>();
            if !owner_lines.is_empty() {
                display_str.push_str(&format!("<ul>{}</ul>", owner_lines.join("")));
            }
        }
        display_str.push_str("</li>");
    }
    display_str.push_str("</ul>");

    display_str
}
