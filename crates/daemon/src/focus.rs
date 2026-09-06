//! Compositor window identities. Window titles and document contents are never logged.

use serde_json::Value;
use tokio::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    pub id: String,
    pub app_id: String,
}

impl Window {
    pub fn copy_combo(&self) -> &'static str {
        if self.is_terminal() {
            "ctrl+shift+c"
        } else {
            "ctrl+c"
        }
    }

    pub fn paste_combo(&self) -> &'static str {
        if self.is_terminal() {
            "ctrl+shift+v"
        } else {
            "ctrl+v"
        }
    }

    fn is_terminal(&self) -> bool {
        [
            "com.mitchellh.ghostty",
            "kitty",
            "foot",
            "footclient",
            "alacritty",
            "org.wezfurlong.wezterm",
            "wezterm",
            "gnome-terminal-server",
            "org.gnome.terminal",
            "org.gnome.console",
            "org.gnome.ptyxis",
            "ptyxis",
            "konsole",
            "org.kde.konsole",
            "xterm",
            "urxvt",
            "xfce4-terminal",
            "tilix.tilix",
            "io.elementary.terminal",
            "com.raggesilver.blackbox",
        ]
        .iter()
        .any(|name| self.app_id.eq_ignore_ascii_case(name))
    }
}

pub async fn current() -> Option<Window> {
    let (program, args, compositor) = if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some() {
        ("hyprctl", vec!["activewindow", "-j"], "hyprland")
    } else if std::env::var_os("SWAYSOCK").is_some() {
        ("swaymsg", vec!["-t", "get_tree", "-r"], "sway")
    } else if std::env::var_os("NIRI_SOCKET").is_some() {
        ("niri", vec!["msg", "--json", "focused-window"], "niri")
    } else {
        return None;
    };
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        Command::new(program).args(args).kill_on_drop(true).output(),
    )
    .await
    .ok()?
    .ok()?;
    if !output.status.success() {
        return None;
    }
    parse(compositor, &serde_json::from_slice(&output.stdout).ok()?)
}

fn parse(compositor: &str, value: &Value) -> Option<Window> {
    let (id, app_id) = match compositor {
        "hyprland" => {
            let address = value["address"].as_str()?;
            if address.is_empty() || address == "0x0" {
                return None;
            }
            (
                address.to_owned(),
                value["class"].as_str().unwrap_or_default(),
            )
        }
        "niri" => (
            value["id"].as_u64()?.to_string(),
            value["app_id"].as_str().unwrap_or_default(),
        ),
        "sway" => {
            if value["focused"].as_bool() != Some(true) {
                return ["nodes", "floating_nodes"]
                    .iter()
                    .filter_map(|key| value[key].as_array())
                    .flatten()
                    .find_map(|node| parse(compositor, node));
            }
            // An empty workspace can be focused too. It isn't a paste target.
            if value["type"].as_str() != Some("con") {
                return None;
            }
            (
                value["id"].as_u64()?.to_string(),
                value["app_id"]
                    .as_str()
                    .or_else(|| value["window_properties"]["class"].as_str())
                    .unwrap_or_default(),
            )
        }
        _ => return None,
    };
    Some(Window {
        id: format!("{compositor}:{id}"),
        app_id: app_id.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn identifies_windows_without_using_titles_or_shared_app_ids() {
        let a = parse("hyprland", &json!({"address":"0xab", "class":"kitty"})).unwrap();
        let b = parse("hyprland", &json!({"address":"0xcd", "class":"kitty"})).unwrap();
        assert_ne!(a, b);
        assert_eq!(a.copy_combo(), "ctrl+shift+c");
        assert_eq!(a.paste_combo(), "ctrl+shift+v");
        assert!(parse("hyprland", &json!({})).is_none());
        assert!(parse("niri", &Value::Null).is_none());
        assert_eq!(
            parse("niri", &json!({"id":12,"app_id":"firefox"}))
                .unwrap()
                .paste_combo(),
            "ctrl+v"
        );
    }

    #[test]
    fn finds_sway_floating_xwayland_windows_and_ignores_empty_workspaces() {
        let tree = json!({"nodes":[{"type":"workspace", "nodes":[], "floating_nodes":[
            {"type":"con", "id":42, "focused":true, "app_id":null, "window_properties":{"class":"Alacritty"}}
        ]}]});
        let window = parse("sway", &tree).unwrap();
        assert_eq!(window.id, "sway:42");
        assert_eq!(window.paste_combo(), "ctrl+shift+v");
        assert!(parse("sway", &json!({"id":1,"type":"workspace","focused":true})).is_none());
    }
}
