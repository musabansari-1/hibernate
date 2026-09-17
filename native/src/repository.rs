//! Read-only repository context. No Git commands, hooks, checkout or index writes.
use serde_json::{json, Value};
use std::{io::Read, path::Path};
fn small_file(p: &Path) -> Option<String> {
    let f = std::fs::File::open(p).ok()?;
    if f.metadata().ok()?.len() > 131072 {
        return None;
    }
    let mut s = String::new();
    f.take(131072).read_to_string(&mut s).ok()?;
    Some(s)
}
pub fn inspect(raw: &str) -> Result<Value, String> {
    let path = crate::context::local_folder(raw)?;
    for root in path.ancestors().take(32) {
        let marker = root.join(".git");
        let git = if marker.is_dir() {
            marker
        } else if marker.is_file() {
            let text = small_file(&marker).ok_or("Cannot read Git worktree pointer")?;
            root.join(
                text.trim()
                    .strip_prefix("gitdir: ")
                    .ok_or("Invalid Git worktree pointer")?,
            )
        } else {
            continue;
        };
        let git = crate::context::local_folder(&git.to_string_lossy())?;
        let head = small_file(&git.join("HEAD")).unwrap_or_default();
        let branch = head
            .trim()
            .strip_prefix("ref: refs/heads/")
            .map(str::to_string);
        let commit = if branch.is_none()
            && head.trim().len() == 40
            && head.trim().chars().all(|c| c.is_ascii_hexdigit())
        {
            Some(head.trim().to_string())
        } else {
            None
        };
        let common = small_file(&git.join("commondir"))
            .map(|s| git.join(s.trim()))
            .unwrap_or(git.clone());
        let common = crate::context::local_folder(&common.to_string_lossy())?;
        let config = small_file(&common.join("config")).unwrap_or_default();
        let mut in_origin = false;
        let mut repository = None;
        for line in config.lines() {
            let line = line.trim();
            if line.starts_with('[') {
                in_origin = line == "[remote \"origin\"]";
            } else if in_origin {
                if let Some((key, value)) = line.split_once('=') {
                    if key.trim() == "url" {
                        let raw = value.trim();
                        let candidate = raw
                            .strip_prefix("git@github.com:")
                            .map(str::to_string)
                            .or_else(|| {
                                url::Url::parse(raw)
                                    .ok()
                                    .filter(|u| u.host_str() == Some("github.com"))
                                    .map(|u| u.path().trim_start_matches('/').to_string())
                            });
                        if let Some(candidate) = candidate {
                            let candidate =
                                candidate.trim_end_matches(".git").trim_end_matches('/');
                            if candidate.split('/').count() == 2
                                && candidate
                                    .chars()
                                    .all(|c| c.is_ascii_alphanumeric() || "-_/ .".contains(c))
                                && !candidate.contains(' ')
                            {
                                repository = Some(candidate.to_string());
                            }
                        }
                    }
                }
            }
        }
        return Ok(
            json!({"root":root.to_string_lossy(),"branch":branch,"detachedCommit":commit,"repository":repository,"workingTree":"unverified","detail":"Repository and HEAD observed without running Git. Working-tree changes are not scanned or modified.","capturedAt":crate::now()}),
        );
    }
    Ok(
        json!({"root":null,"detail":"No Git repository found in the project folder or its parents."}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_worktree_is_read_but_remote_git_pointer_is_refused() {
        let p = std::env::temp_dir().join(format!("hibernate-worktree-{}", uuid::Uuid::new_v4()));
        let work = p.join("work");
        let metadata = p.join("metadata");
        std::fs::create_dir_all(&work).unwrap();
        std::fs::create_dir_all(&metadata).unwrap();
        std::fs::write(work.join(".git"), "gitdir: ../metadata\n").unwrap();
        std::fs::write(metadata.join("HEAD"), "ref: refs/heads/worktree\n").unwrap();
        assert_eq!(
            inspect(work.to_str().unwrap()).unwrap()["branch"],
            "worktree"
        );
        std::fs::write(work.join(".git"), "gitdir: \\\\server\\share\n").unwrap();
        assert!(inspect(work.to_str().unwrap()).is_err());
        std::fs::write(work.join(".git"), "gitdir: ../metadata\n").unwrap();
        std::fs::write(metadata.join("commondir"), "\\\\server\\share\n").unwrap();
        assert!(inspect(work.to_str().unwrap()).is_err());
    }
    #[test]
    fn reads_branch_and_removes_remote_credentials() {
        let p = std::env::temp_dir().join(format!("hibernate-git-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(p.join(".git")).unwrap();
        std::fs::write(p.join(".git/HEAD"), "ref: refs/heads/feature/restore\n").unwrap();
        std::fs::write(
            p.join(".git/config"),
            "[remote \"origin\"]\nurl = https://token:secret@github.com/acme/project.git\n",
        )
        .unwrap();
        let value = inspect(p.to_str().unwrap()).unwrap();
        assert_eq!(value["branch"], "feature/restore");
        assert_eq!(value["repository"], "acme/project");
        assert!(!value.to_string().contains("secret"));
    }
}
