use std::{env, fs, path::PathBuf};

use zed::settings::LspSettings;
use zed_extension_api::{self as zed, LanguageServerId, Result};

const LANGUAGE_SERVER_ID: &str = "gsx";
const BINARY_NAME: &str = "gsx";
const GO_INSTALL_PACKAGE: &str = "github.com/gsxhq/gsx/cmd/gsx@latest";

struct GsxExtension {
    cached_binary_path: Option<String>,
}

impl GsxExtension {
    fn language_server_settings(worktree: &zed::Worktree) -> LspSettings {
        LspSettings::for_worktree(LANGUAGE_SERVER_ID, worktree).unwrap_or_default()
    }

    fn find_language_server_binary(
        &mut self,
        language_server_id: &LanguageServerId,
        settings: &LspSettings,
        worktree: &zed::Worktree,
        env: &zed::EnvVars,
    ) -> Result<String> {
        if let Some(path) = &self.cached_binary_path {
            if let Ok(path) = self.verify_binary(path, env) {
                return Ok(path);
            }
        }

        if let Some(path) = settings
            .binary
            .as_ref()
            .and_then(|binary| binary.path.as_ref())
            .filter(|path| !path.is_empty())
        {
            return self.verify_binary(path, env);
        }

        if let Some(path) = worktree.which(BINARY_NAME) {
            if let Ok(path) = self.verify_binary(&path, env) {
                self.cached_binary_path = Some(path.clone());
                return Ok(path);
            }
        }

        for path in self.go_bin_candidates(env) {
            if let Ok(path) = self.verify_binary(&path, env) {
                self.cached_binary_path = Some(path.clone());
                return Ok(path);
            }
        }

        self.install_language_server(language_server_id, env, worktree)
    }

    fn command_env(&self, settings: &LspSettings, worktree: &zed::Worktree) -> zed::EnvVars {
        let mut env = worktree.shell_env();

        if let Some(extra_env) = settings
            .binary
            .as_ref()
            .and_then(|binary| binary.env.as_ref())
        {
            env.extend(extra_env.clone());
        }

        env
    }

    fn language_server_env(
        &self,
        settings: &LspSettings,
        worktree: &zed::Worktree,
    ) -> zed::EnvVars {
        let mut env = self.command_env(settings, worktree);
        prepend_go_mod_toolchain(&mut env, worktree);
        env
    }

    fn verify_binary(&self, path: &str, env: &zed::EnvVars) -> Result<String> {
        let mut command = zed::process::Command::new(path)
            .arg("version")
            .envs(env.clone());
        let output = command
            .output()
            .map_err(|error| format!("failed to run `{path} version`: {error}"))?;
        let stdout = String::from_utf8_lossy(&output.stdout);

        if output.status == Some(0) && stdout.trim_start().starts_with("gsx ") {
            Ok(path.to_string())
        } else {
            Err(format!(
                "`{path} version` did not look like the GSX compiler; stdout was `{}`",
                stdout.trim()
            ))
        }
    }

    fn go_bin_candidates(&self, env: &zed::EnvVars) -> Vec<String> {
        let mut candidates = Vec::new();

        if let Some(gobin) = env_var(env, "GOBIN") {
            push_candidate(&mut candidates, join_path(gobin, binary_filename()));
        }

        if let Some(gopath) = env_var(env, "GOPATH") {
            for path in gopath
                .split(path_list_separator())
                .filter(|path| !path.is_empty())
            {
                push_candidate(
                    &mut candidates,
                    join_path(&join_path(path, "bin"), binary_filename()),
                );
            }
        }

        if let Some(home) = env_var(env, "HOME") {
            push_candidate(
                &mut candidates,
                join_path(&join_path(home, "go/bin"), binary_filename()),
            );
        }

        if let Some(go) = go_command_from_path(env).or_else(|| common_go_command(env)) {
            let mut command = zed::process::Command::new(go)
                .args(["env", "GOBIN", "GOPATH"])
                .envs(env.clone());

            if let Ok(output) = command.output() {
                if output.status == Some(0) {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let mut lines = stdout.lines();

                    if let Some(gobin) = lines.next().filter(|line| !line.is_empty()) {
                        push_candidate(&mut candidates, join_path(gobin, binary_filename()));
                    }

                    if let Some(gopath) = lines.next().filter(|line| !line.is_empty()) {
                        for path in gopath
                            .split(path_list_separator())
                            .filter(|path| !path.is_empty())
                        {
                            push_candidate(
                                &mut candidates,
                                join_path(&join_path(path, "bin"), binary_filename()),
                            );
                        }
                    }
                }
            }
        }

        candidates
    }

    fn install_language_server(
        &mut self,
        language_server_id: &LanguageServerId,
        env: &zed::EnvVars,
        worktree: &zed::Worktree,
    ) -> Result<String> {
        let go = self.go_command(env, worktree).ok_or_else(|| {
            concat!(
                "Could not find the GSX language server or the Go toolchain. ",
                "Install GSX with `go install github.com/gsxhq/gsx/cmd/gsx@latest`, ",
                "or configure `lsp.gsx.binary.path` to the absolute path of the `gsx` binary."
            )
            .to_string()
        })?;

        zed::set_language_server_installation_status(
            language_server_id,
            &zed::LanguageServerInstallationStatus::CheckingForUpdate,
        );

        let install_dir = env::current_dir()
            .map_err(|error| format!("failed to get extension working directory: {error}"))?
            .join("gsx-bin");
        fs::create_dir_all(&install_dir).map_err(|error| {
            format!(
                "failed to create GSX language server install directory '{}': {error}",
                install_dir.to_string_lossy()
            )
        })?;

        let binary_path = install_dir
            .join(binary_filename())
            .to_string_lossy()
            .to_string();
        if let Ok(path) = self.verify_binary(&binary_path, env) {
            self.cached_binary_path = Some(path.clone());
            return Ok(path);
        }

        zed::set_language_server_installation_status(
            language_server_id,
            &zed::LanguageServerInstallationStatus::Downloading,
        );

        let mut install_env = env.clone();
        install_env.push((
            "GOBIN".to_string(),
            install_dir.to_string_lossy().to_string(),
        ));

        let mut command = zed::process::Command::new(go)
            .args(["install", GO_INSTALL_PACKAGE])
            .envs(install_env.clone());
        let output = command
            .output()
            .map_err(|error| format!("failed to run `go install {GO_INSTALL_PACKAGE}`: {error}"))?;

        if output.status != Some(0) {
            return Err(format!(
                "`go install {GO_INSTALL_PACKAGE}` failed:\n{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let path = self.verify_binary(&binary_path, &install_env)?;
        self.cached_binary_path = Some(path.clone());
        Ok(path)
    }

    fn go_command(&self, env: &zed::EnvVars, worktree: &zed::Worktree) -> Option<String> {
        env_var(env, "GOCMD")
            .map(ToOwned::to_owned)
            .filter(|path| verify_go_command(path, env))
            .or_else(|| go_command_from_path(env))
            .or_else(|| common_go_command(env))
            .or_else(|| {
                worktree
                    .which("go")
                    .filter(|path| !is_project_go_wrapper(path))
                    .filter(|path| verify_go_command(path, env))
            })
    }
}

impl zed::Extension for GsxExtension {
    fn new() -> Self {
        Self {
            cached_binary_path: None,
        }
    }

    fn language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        let settings = Self::language_server_settings(worktree);
        let env = self.language_server_env(&settings, worktree);
        let binary =
            self.find_language_server_binary(language_server_id, &settings, worktree, &env)?;
        let binary_settings = settings.binary.unwrap_or_else(default_binary_settings);

        Ok(zed::Command {
            command: binary,
            args: binary_settings
                .arguments
                .unwrap_or_else(|| vec!["lsp".to_string()]),
            env: env.into_iter().collect(),
        })
    }

    fn language_server_initialization_options(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<Option<zed::serde_json::Value>> {
        Ok(Self::language_server_settings(worktree).initialization_options)
    }

    fn language_server_workspace_configuration(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<Option<zed::serde_json::Value>> {
        Ok(Self::language_server_settings(worktree).settings)
    }
}

fn default_binary_settings() -> zed::settings::CommandSettings {
    zed::settings::CommandSettings {
        path: None,
        arguments: None,
        env: None,
    }
}

fn env_var<'a>(env: &'a zed::EnvVars, name: &str) -> Option<&'a str> {
    env.iter()
        .rev()
        .find(|(key, value)| key == name && !value.is_empty())
        .map(|(_, value)| value.as_str())
}

fn go_command_from_path(env: &zed::EnvVars) -> Option<String> {
    env_var(env, "PATH").and_then(|path| {
        path.split(path_list_separator())
            .filter(|dir| !dir.is_empty())
            .map(|dir| join_path(dir, go_binary_filename()))
            .filter(|path| !is_project_go_wrapper(path))
            .find(|path| verify_go_command(path, env))
    })
}

fn is_project_go_wrapper(path: &str) -> bool {
    let normalized = path.replace('\\', "/");
    normalized.ends_with("/cmd/go/go") || normalized.ends_with("/cmd/go/go.exe")
}

fn common_go_command(env: &zed::EnvVars) -> Option<String> {
    [
        "/opt/homebrew/bin/go",
        "/usr/local/bin/go",
        "/usr/bin/go",
        "/opt/local/bin/go",
    ]
    .into_iter()
    .find(|path| verify_go_command(path, env))
    .map(ToOwned::to_owned)
}

fn prepend_go_mod_toolchain(env: &mut zed::EnvVars, worktree: &zed::Worktree) {
    let Some(version) = worktree
        .read_text_file("go.mod")
        .ok()
        .and_then(|go_mod| go_mod_toolchain_version(&go_mod))
    else {
        return;
    };

    let Some(platform) = go_toolchain_platform() else {
        return;
    };

    if let Some(toolchain_bin) = go_mod_cache_candidates(env)
        .into_iter()
        .find_map(|mod_cache| {
            let bin = PathBuf::from(mod_cache)
                .join(format!("golang.org/toolchain@v0.0.1-{version}.{platform}"))
                .join("bin")
                .to_string_lossy()
                .to_string();
            verify_go_command(&join_path(&bin, go_binary_filename()), env).then_some(bin)
        })
    {
        prepend_path(env, &toolchain_bin);
    }
}

fn go_mod_toolchain_version(go_mod: &str) -> Option<String> {
    let mut go_version = None;

    for (directive, value) in go_mod.lines().filter_map(go_mod_directive) {
        match directive {
            "toolchain" if value.starts_with("go") => return Some(value.to_string()),
            "go" if go_version.is_none() => go_version = Some(format!("go{value}")),
            _ => {}
        }
    }

    go_version
}

fn go_mod_directive(line: &str) -> Option<(&str, &str)> {
    let line = line.split("//").next()?.trim();
    let mut fields = line.split_whitespace();
    Some((fields.next()?, fields.next()?))
}

fn go_mod_cache_candidates(env: &zed::EnvVars) -> Vec<String> {
    let mut candidates = Vec::new();

    if let Some(gomodcache) = env_var(env, "GOMODCACHE") {
        candidates.push(gomodcache.to_string());
    }

    if let Some(gopath) = env_var(env, "GOPATH") {
        for path in gopath
            .split(path_list_separator())
            .filter(|path| !path.is_empty())
        {
            candidates.push(join_path(&join_path(path, "pkg"), "mod"));
        }
    }

    if let Some(home) = env_var(env, "HOME") {
        candidates.push(join_path(&join_path(home, "go/pkg"), "mod"));
    }

    candidates
}

fn go_toolchain_platform() -> Option<&'static str> {
    let (os, architecture) = zed::current_platform();
    match (os, architecture) {
        (zed::Os::Mac, zed::Architecture::Aarch64) => Some("darwin-arm64"),
        (zed::Os::Mac, zed::Architecture::X8664) => Some("darwin-amd64"),
        (zed::Os::Linux, zed::Architecture::Aarch64) => Some("linux-arm64"),
        (zed::Os::Linux, zed::Architecture::X8664) => Some("linux-amd64"),
        (zed::Os::Windows, zed::Architecture::X8664) => Some("windows-amd64"),
        (zed::Os::Windows, zed::Architecture::X86) => Some("windows-386"),
        _ => None,
    }
}

fn prepend_path(env: &mut zed::EnvVars, path: &str) {
    let path = match env_var(env, "PATH") {
        Some(existing) if !existing.is_empty() => {
            format!("{path}{}{existing}", path_list_separator())
        }
        _ => path.to_string(),
    };
    env.push(("PATH".to_string(), path));
}

fn push_candidate(candidates: &mut Vec<String>, path: String) {
    if !candidates.contains(&path) {
        candidates.push(path);
    }
}

fn verify_go_command(path: &str, env: &zed::EnvVars) -> bool {
    let mut command = zed::process::Command::new(path)
        .arg("version")
        .envs(env.clone());
    command.output().is_ok_and(|output| {
        output.status == Some(0)
            && String::from_utf8_lossy(&output.stdout).starts_with("go version ")
    })
}

fn binary_filename() -> &'static str {
    match zed::current_platform().0 {
        zed::Os::Windows => "gsx.exe",
        zed::Os::Mac | zed::Os::Linux => BINARY_NAME,
    }
}

fn go_binary_filename() -> &'static str {
    match zed::current_platform().0 {
        zed::Os::Windows => "go.exe",
        zed::Os::Mac | zed::Os::Linux => "go",
    }
}

fn path_list_separator() -> char {
    match zed::current_platform().0 {
        zed::Os::Windows => ';',
        zed::Os::Mac | zed::Os::Linux => ':',
    }
}

fn join_path(path: &str, segment: &str) -> String {
    PathBuf::from(path)
        .join(segment)
        .to_string_lossy()
        .to_string()
}

zed::register_extension!(GsxExtension);
