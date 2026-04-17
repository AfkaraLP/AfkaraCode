use std::env;
use std::fs;
use std::path::{PathBuf};

pub struct XdgPaths {
    pub config_home: PathBuf,
    pub config_dirs: Vec<PathBuf>,
    pub data_home: PathBuf,
    pub data_dirs: Vec<PathBuf>,
    pub state_home: PathBuf,
    pub cache_home: PathBuf,
    pub runtime_dir: Option<PathBuf>,
}

fn home_dir() -> PathBuf {
    if let Ok(h) = env::var("HOME") { return PathBuf::from(h); }
    if let Ok(h) = env::var("USERPROFILE") { return PathBuf::from(h); }
    env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

pub fn xdg_paths() -> XdgPaths {
    let home = home_dir();

    let config_home = env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home.join(".config"));
    let config_dirs = env::var("XDG_CONFIG_DIRS").unwrap_or_else(|_| "/etc/xdg".to_string());
    let config_dirs: Vec<PathBuf> = config_dirs.split(':').filter(|s| !s.is_empty()).map(PathBuf::from).collect();

    let data_home = env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home.join(".local").join("share"));
    let data_dirs = env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".to_string());
    let data_dirs: Vec<PathBuf> = data_dirs.split(':').filter(|s| !s.is_empty()).map(PathBuf::from).collect();

    let state_home = env::var("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home.join(".local").join("state"));
    let cache_home = env::var("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home.join(".cache"));
    let runtime_dir = env::var("XDG_RUNTIME_DIR").ok().map(PathBuf::from);

    XdgPaths {
        config_home,
        config_dirs,
        data_home,
        data_dirs,
        state_home,
        cache_home,
        runtime_dir,
    }
}

pub fn ensure_app_dirs() -> (PathBuf, PathBuf, PathBuf, PathBuf) {
    let x = xdg_paths();
    let app = "afkaracode";

    let cfg = x.config_home.join(app);
    let data = x.data_home.join(app);
    let state = x.state_home.join(app);
    let cache = x.cache_home.join(app);

    let _ = fs::create_dir_all(&cfg);
    let _ = fs::create_dir_all(&data);
    let _ = fs::create_dir_all(&state);
    let _ = fs::create_dir_all(&cache);

    (cfg, data, state, cache)
}

pub fn plugin_dirs() -> Vec<PathBuf> {
    // Ensure base dirs exist and ensure plugin dir at config_home exists.
    let (cfg, _data, _state, _cache) = ensure_app_dirs();
    let plugins_cfg = cfg.join("plugins");
    let _ = fs::create_dir_all(&plugins_cfg);

    let mut dirs = vec![plugins_cfg];

    // Add each XDG_CONFIG_DIRS/afkaracode/plugins if exists
    let x = xdg_paths();
    for d in x.config_dirs {
        let p = d.join("afkaracode").join("plugins");
        if p.exists() { dirs.push(p); }
    }

    dirs
}
