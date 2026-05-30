//! XDG directory resolution and `$VAR` expansion for paths in config.

use std::path::PathBuf;

pub fn expand(s: &str) -> PathBuf {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' {
            // peek var name (alphanumeric + underscore)
            let start = i + 1;
            let mut end = start;
            while end < bytes.len()
                && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_')
            {
                end += 1;
            }
            if end > start {
                let key = &s[start..end];
                let resolved = resolve_var(key);
                out.push_str(&resolved);
                i = end;
                continue;
            }
        }
        out.push(s.as_bytes()[i] as char);
        i += 1;
    }
    PathBuf::from(out)
}

fn resolve_var(name: &str) -> String {
    if let Ok(v) = std::env::var(name) {
        return v;
    }
    // XDG defaults per spec
    let home = std::env::var("HOME").unwrap_or_default();
    match name {
        "XDG_CONFIG_HOME"  => format!("{home}/.config"),
        "XDG_DATA_HOME"    => format!("{home}/.local/share"),
        "XDG_STATE_HOME"   => format!("{home}/.local/state"),
        "XDG_RUNTIME_DIR"  => format!("/run/user/{}", nix_uid()),
        _                  => format!("${name}"), // leave unknown vars literal
    }
}

fn nix_uid() -> String {
    // SAFETY: getuid is always safe.
    unsafe { libc::getuid() }.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Serialize env-mutating tests in this module.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct EnvGuard {
        keys: Vec<(&'static str, Option<String>)>,
    }
    impl EnvGuard {
        fn capture(keys: &[&'static str]) -> Self {
            Self {
                keys: keys.iter().map(|k| (*k, std::env::var(k).ok())).collect(),
            }
        }
    }
    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (k, v) in &self.keys {
                match v {
                    Some(val) => std::env::set_var(k, val),
                    None      => std::env::remove_var(k),
                }
            }
        }
    }

    #[test]
    fn expands_xdg_config_home_default() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _g = EnvGuard::capture(&["XDG_CONFIG_HOME", "HOME"]);
        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::set_var("HOME", "/home/tester");
        let p = expand("$XDG_CONFIG_HOME/smarty-pants/config.toml");
        assert_eq!(p, PathBuf::from("/home/tester/.config/smarty-pants/config.toml"));
    }

    #[test]
    fn explicit_xdg_var_wins_over_default() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _g = EnvGuard::capture(&["XDG_CONFIG_HOME"]);
        std::env::set_var("XDG_CONFIG_HOME", "/somewhere/else");
        let p = expand("$XDG_CONFIG_HOME/smarty-pants");
        assert_eq!(p, PathBuf::from("/somewhere/else/smarty-pants"));
    }

    #[test]
    fn leaves_unknown_vars_literal() {
        // This test doesn't mutate env vars but still goes through resolve_var,
        // and resolve_var reads $NOT_A_VAR. Lock anyway to be safe.
        let _lock = ENV_LOCK.lock().unwrap();
        let p = expand("$NOT_A_VAR/x");
        assert_eq!(p, PathBuf::from("$NOT_A_VAR/x"));
    }

    #[test]
    fn xdg_runtime_dir_uses_getuid_when_unset() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _g = EnvGuard::capture(&["XDG_RUNTIME_DIR"]);
        std::env::remove_var("XDG_RUNTIME_DIR");
        let p = expand("$XDG_RUNTIME_DIR/smarty-pants.sock");
        let uid = unsafe { libc::getuid() };
        assert_eq!(p, PathBuf::from(format!("/run/user/{uid}/smarty-pants.sock")));
    }
}
