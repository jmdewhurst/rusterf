use std::path::{Path, PathBuf};

macro_rules! tomlget_or {
    ($cfg:ident, $sec:expr, $key:expr, $conv:ident, $as:ty, $or:expr) => {
        $cfg.get($sec)
            .and_then(|sec| sec.get($key))
            .map(|val| val.$conv())
            .unwrap_or_else(|| {
                eprintln!(
                    "failed to find {}:{} in config; proceeding with default {:?}",
                    $sec, $key, $or
                );
                Some($or)
            })
            .unwrap_or_else(|| {
                eprintln!(
                    "failed to convert {}:{} to {}; proceeding with default {:?}",
                    $sec,
                    $key,
                    stringify!($as),
                    $or
                );
                $or
            }) as $as
    };
    ($cfg:ident, $sec:expr, $key:expr, as_str, $or:expr) => {
        $cfg.get($sec)
            .and_then(|sec| sec.get($key))
            .map(|val| val.as_str())
            .unwrap_or_else(|| {
                eprintln!(
                    "failed to get {}:{} in config; proceeding with default {:?}",
                    $sec, $key, $or
                );
                Some($or)
            })
            .unwrap_or_else(|| {
                eprintln!(
                    "failed to convert {}:{} to string; proceeding with default {:?}",
                    $sec, $key, $or
                );
                $or
            })
    };
    ($cfg:ident, $sec:expr, $key:expr, as_bool, $or:expr) => {
        $cfg.get($sec)
            .and_then(|sec| sec.get($key))
            .map(|val| val.as_bool())
            .unwrap_or_else(|| {
                eprintln!(
                    "failed to convert {}:{} to bool; proceeding with default {:?}",
                    $sec, $key, $or
                );
                Some($or)
            })
            .unwrap_or_else(|| {
                eprintln!(
                    "failed to get {}:{} in config; proceeding with default {:?}",
                    $sec, $key, $or
                );
                $or
            })
    };
}

macro_rules! tomlget_opt {
    ($cfg:ident, $sec:expr, $key:expr, $conv:ident, $as:ty) => {
        $cfg.get($sec)
            .and_then(|sec| sec.get($key))
            .and_then(|val| val.$conv())
            .map(|val| val as $as)
    };
    ($cfg:ident, $sec:expr, $key:expr, as_str) => {
        $cfg.get($sec)
            .and_then(|sec| sec.get($key))
            .and_then(|val| val.as_str())
    };
    ($cfg:ident, $sec:expr, $key:expr, as_bool, $or:expr) => {
        $cfg.get($sec)
            .and_then(|sec| sec.get($key))
            .and_then(|val| val.as_bool())
    };
}

macro_rules! tomlget {
    ($cfg:ident, $sec:expr, $key:expr, $conv:ident, $as:ty) => {
        $cfg.get($sec)
            .ok_or_else(|| format!("failed to get section {}", $sec))?
            .get($key)
            .ok_or_else(|| format!("failed to get key {}:{}", $sec, $key))?
            .$conv()
            .ok_or_else(|| format!("failed to convert {}:{} to {}", $sec, $key, stringify!($as)))?
            as $as
    };
    ($cfg:ident, $sec:expr, $key:expr, as_str) => {
        $cfg.get($sec)
            .ok_or_else(|| format!("failed to get section {}", $sec))?
            .get($key)
            .ok_or_else(|| format!("failed to get key {}:{}", $sec, $key))?
            .as_str()
            .ok_or_else(|| format!("failed to convert {}:{} to string", $sec, $key))?
    };
    ($cfg:ident, $sec:expr, $key:expr, as_bool) => {
        $cfg.get($sec)
            .ok_or_else(|| format!("failed to get section {}", $sec))?
            .get($key)
            .ok_or_else(|| format!("failed to get key {}:{}", $sec, $key))?
            .as_bool()
            .ok_or_else(|| format!("failed to convert {}:{} to boolean", $sec, $key))?
    };
}

pub fn find_file(file_name: &Path) -> Option<PathBuf> {
    if file_name.is_absolute() {
        if file_name.exists() {
            return Some(file_name.into());
        } else {
            return None;
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        if cwd.join(file_name).exists() {
            return Some(cwd.join(file_name));
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if exe.parent()?.join(file_name).exists() {
            return Some(exe.parent()?.join(file_name));
        }
    }
    None
}

pub(crate) use {tomlget, tomlget_opt, tomlget_or};
//
//
//
//
