use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const BACKEND_ROOT_ENV: &str = "RIICHI_AI_TRAINER_BACKEND_ROOT";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendResourceSource {
    EnvOverride,
    BundledResource,
    ExeAdjacent,
    RepoRoot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendResourceStatus {
    pub status: String,
    pub source: BackendResourceSource,
    pub backend_project: PathBuf,
    pub working_dir: PathBuf,
    pub uv_path: PathBuf,
    #[serde(default)]
    pub reasons: Vec<String>,
    #[serde(default)]
    pub attempted_roots: Vec<PathBuf>,
}

pub fn locate_backend_resource(
    resource_dir: Option<&Path>,
) -> Result<BackendResourceStatus, String> {
    let cwd =
        std::env::current_dir().map_err(|error| format!("read current dir failed: {error}"))?;
    locate_backend_resource_from(resource_dir, &cwd)
}

pub fn locate_backend_resource_from(
    resource_dir: Option<&Path>,
    cwd: &Path,
) -> Result<BackendResourceStatus, String> {
    let uv_path = locate_uv_path();
    let mut attempts = Vec::new();

    if let Some(env_root) = std::env::var_os(BACKEND_ROOT_ENV).map(PathBuf::from) {
        if let Some(status) = status_for_candidate(
            BackendResourceSource::EnvOverride,
            &env_root,
            uv_path.clone(),
            &mut attempts,
        ) {
            return Ok(status);
        }
    }

    if let Some(resource_dir) = resource_dir {
        if let Some(status) = status_for_candidate(
            BackendResourceSource::BundledResource,
            &resource_dir.join("backend"),
            uv_path.clone(),
            &mut attempts,
        ) {
            return Ok(status);
        }
    }

    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
    {
        if let Some(status) = status_for_candidate(
            BackendResourceSource::ExeAdjacent,
            &exe_dir.join("backend"),
            uv_path.clone(),
            &mut attempts,
        ) {
            return Ok(status);
        }
    }

    if let Some(repo_root) = find_repo_root_from(cwd) {
        if let Some(status) = status_for_candidate(
            BackendResourceSource::RepoRoot,
            &repo_root.join("backend"),
            uv_path,
            &mut attempts,
        ) {
            return Ok(status);
        }
    }

    Err(format!(
        "could not locate backend/pyproject.toml; attempted roots: {}",
        attempts
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

pub fn find_repo_root_from(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        if is_repo_root(&current) {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

fn status_for_candidate(
    source: BackendResourceSource,
    candidate: &Path,
    uv_path: PathBuf,
    attempts: &mut Vec<PathBuf>,
) -> Option<BackendResourceStatus> {
    attempts.push(candidate.to_path_buf());
    let backend_project = normalize_backend_project(candidate)?;
    let working_dir = backend_project
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| backend_project.clone());
    Some(BackendResourceStatus {
        status: "ready".into(),
        source,
        backend_project,
        working_dir,
        uv_path,
        reasons: Vec::new(),
        attempted_roots: attempts.clone(),
    })
}

fn normalize_backend_project(path: &Path) -> Option<PathBuf> {
    if is_backend_project(path) {
        return Some(path.to_path_buf());
    }
    let nested = path.join("backend");
    if is_backend_project(&nested) {
        return Some(nested);
    }
    None
}

fn is_repo_root(path: &Path) -> bool {
    is_backend_project(&path.join("backend"))
}

fn is_backend_project(path: &Path) -> bool {
    path.join("pyproject.toml").is_file() && path.join("src").join("riichi_ai_trainer").is_dir()
}

fn locate_uv_path() -> PathBuf {
    which::which("uv").unwrap_or_else(|_| PathBuf::from("uv"))
}
