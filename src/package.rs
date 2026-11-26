#[cfg(target_arch = "wasm32")]
use std::path::{Path, PathBuf};

pub const MANIFEST: &str = "khipu.json";

#[cfg(not(target_arch = "wasm32"))]
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

#[cfg(target_arch = "wasm32")]
pub fn locate(_: &Path, name: &str) -> Result<PathBuf, String> {
    Err(format!(
        "packages like '{name}' are not available in the playground"
    ))
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::{command, locate, main_file};

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use serde_json::{Map, Value as Json, json};

    use super::{MANIFEST, valid_name};

    const LOCK: &str = "khipu.lock";
    const STORE: &str = ".khipu/packages";

    fn read_json(path: &Path) -> Result<Map<String, Json>, String> {
        let text = fs::read_to_string(path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        match serde_json::from_str(&text) {
            Ok(Json::Object(map)) => Ok(map),
            Ok(_) => Err(format!("{} should hold a JSON object", path.display())),
            Err(error) => Err(format!("{} is not valid JSON: {error}", path.display())),
        }
    }

    fn write_json(path: &Path, value: &Map<String, Json>) -> Result<(), String> {
        let text = serde_json::to_string_pretty(value).unwrap_or_default() + "\n";
        fs::write(path, text).map_err(|error| format!("cannot write {}: {error}", path.display()))
    }

    fn dependencies(manifest: &Map<String, Json>) -> Vec<(String, String)> {
        manifest
            .get("dependencies")
            .and_then(Json::as_object)
            .map(|deps| {
                deps.iter()
                    .filter_map(|(name, source)| Some((name.clone(), source.as_str()?.to_string())))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn root(start: &Path) -> Result<PathBuf, String> {
        let mut dir = start
            .canonicalize()
            .map_err(|error| format!("cannot open {}: {error}", start.display()))?;
        loop {
            if dir.join(MANIFEST).is_file() {
                return Ok(dir);
            }
            if !dir.pop() {
                return Err(format!(
                    "no {MANIFEST} here or in any parent folder; start one with: khipu init"
                ));
            }
        }
    }

    fn entry(package: &Path) -> Result<PathBuf, String> {
        let main = read_json(&package.join(MANIFEST))
            .ok()
            .and_then(|manifest| {
                manifest
                    .get("main")
                    .and_then(Json::as_str)
                    .map(String::from)
            })
            .unwrap_or_else(|| "main.kp".to_string());
        let folder = package
            .canonicalize()
            .map_err(|error| format!("cannot open {}: {error}", package.display()))?;
        let file = folder
            .join(&main)
            .canonicalize()
            .map_err(|_| format!("{} has no {main}", package.display()))?;
        if !file.starts_with(&folder) {
            return Err(format!(
                "{}: main file {main} is outside the package",
                package.display()
            ));
        }
        Ok(file)
    }

    pub fn locate(from: &Path, name: &str) -> Result<PathBuf, String> {
        if !valid_name(name) {
            return Err(format!(
                "'{name}' is not a package name; to import a file, give its path with .kp at the end"
            ));
        }
        let mut dir = from.canonicalize().unwrap_or_else(|_| from.to_path_buf());
        loop {
            let package = dir.join(STORE).join(name);
            if package.exists() {
                return entry(&package);
            }
            if !dir.pop() {
                return Err(format!(
                    "package '{name}' is not installed; add it with: khipu add {name} <git url>"
                ));
            }
        }
    }

    pub fn main_file(cwd: &Path) -> Result<PathBuf, String> {
        entry(&root(cwd)?)
    }

    fn git(args: &[&str], dir: Option<&Path>) -> Result<String, String> {
        let mut command = Command::new("git");
        command.args(["-c", "protocol.ext.allow=never"]);
        if let Some(dir) = dir {
            command.arg("-C").arg(dir);
        }
        let output = command
            .args(args)
            .output()
            .map_err(|error| format!("cannot run git: {error}"))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("git {}: {}", args[0], stderr.trim()));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    fn fetch(
        root: &Path,
        name: &str,
        source: &str,
        pinned: Option<&str>,
    ) -> Result<String, String> {
        let folder = root.join(STORE).join(name);
        fs::create_dir_all(root.join(STORE))
            .map_err(|error| format!("cannot create {STORE}: {error}"))?;
        if let Some(path) = source.strip_prefix("path:") {
            let target = root
                .join(path)
                .canonicalize()
                .map_err(|_| format!("{name}: cannot find the folder {path}"))?;
            if folder.symlink_metadata().is_ok() {
                remove(&folder)?;
            }
            link(&target, &folder)?;
            return Ok("path".to_string());
        }
        let (url, reference) = match source.split_once('#') {
            Some((url, reference)) => (url, Some(reference)),
            None => (source, None),
        };
        if url.starts_with('-') || reference.is_some_and(|r| r.starts_with('-')) {
            return Err(format!("{name}: '{source}' is not a valid source"));
        }
        if folder.join(".git").exists() {
            git(&["fetch", "--quiet", "--tags", "origin"], Some(&folder))?;
        } else {
            if folder.symlink_metadata().is_ok() {
                remove(&folder)?;
            }
            let path = folder.to_string_lossy().into_owned();
            git(&["clone", "--quiet", "--", url, &path], None)?;
        }
        match (pinned, reference) {
            (Some(commit), _) => {
                git(&["checkout", "--quiet", "--detach", commit], Some(&folder))?;
            }
            (None, Some(reference)) => {
                let remote = format!("origin/{reference}");
                if git(&["checkout", "--quiet", "--detach", &remote], Some(&folder)).is_err() {
                    git(
                        &["checkout", "--quiet", "--detach", reference],
                        Some(&folder),
                    )?;
                }
            }
            (None, None) => {
                git(
                    &["checkout", "--quiet", "--detach", "origin/HEAD"],
                    Some(&folder),
                )?;
            }
        }
        git(&["rev-parse", "HEAD"], Some(&folder))
    }

    #[cfg(unix)]
    fn link(target: &Path, folder: &Path) -> Result<(), String> {
        std::os::unix::fs::symlink(target, folder)
            .map_err(|error| format!("cannot link {}: {error}", folder.display()))
    }

    #[cfg(not(unix))]
    fn link(_: &Path, folder: &Path) -> Result<(), String> {
        Err(format!(
            "path: packages are not supported on this system yet ({})",
            folder.display()
        ))
    }

    fn remove(folder: &Path) -> Result<(), String> {
        let meta = folder
            .symlink_metadata()
            .map_err(|error| format!("cannot remove {}: {error}", folder.display()))?;
        let result = if meta.file_type().is_symlink() || meta.is_file() {
            fs::remove_file(folder)
        } else {
            fs::remove_dir_all(folder)
        };
        result.map_err(|error| format!("cannot remove {}: {error}", folder.display()))
    }

    fn install(root: &Path) -> Result<Vec<(String, String)>, String> {
        let manifest = read_json(&root.join(MANIFEST))?;
        let lock = read_json(&root.join(LOCK)).unwrap_or_default();
        let mut wanted = dependencies(&manifest);
        let mut installed: BTreeMap<String, (String, String)> = BTreeMap::new();
        while let Some((name, source)) = wanted.pop() {
            if !valid_name(&name) {
                return Err(format!(
                    "'{name}' is not a valid package name (use lowercase letters, digits, - and _)"
                ));
            }
            if let Some((existing, _)) = installed.get(&name) {
                if *existing != source {
                    return Err(format!(
                        "two packages need '{name}' from different places: {existing} and {source}"
                    ));
                }
                continue;
            }
            let locked = lock.get(&name).and_then(Json::as_object);
            let pinned = locked
                .filter(|entry| entry.get("source").and_then(Json::as_str) == Some(&source))
                .and_then(|entry| entry.get("commit").and_then(Json::as_str));
            let commit = fetch(root, &name, &source, pinned)?;
            if let Ok(inner) = read_json(&root.join(STORE).join(&name).join(MANIFEST)) {
                wanted.extend(dependencies(&inner));
            }
            installed.insert(name, (source, commit));
        }

        let store = root.join(STORE);
        if let Ok(entries) = fs::read_dir(&store) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if !installed.contains_key(&name) {
                    remove(&entry.path())?;
                }
            }
        }
        let mut lock = Map::new();
        for (name, (source, commit)) in &installed {
            lock.insert(name.clone(), json!({ "source": source, "commit": commit }));
        }
        write_json(&root.join(LOCK), &lock)?;
        Ok(installed
            .into_iter()
            .map(|(name, (_, commit))| (name, commit))
            .collect())
    }

    fn report(installed: &[(String, String)]) {
        if installed.is_empty() {
            println!("no dependencies");
        }
        for (name, commit) in installed {
            let at = if commit == "path" {
                "local folder"
            } else {
                &commit[..commit.len().min(10)]
            };
            println!("  {name} ({at})");
        }
    }

    fn init(cwd: &Path) -> Result<(), String> {
        if cwd.join(MANIFEST).exists() {
            return Err(format!("{MANIFEST} already exists here"));
        }
        let name: String = cwd
            .file_name()
            .map(|name| name.to_string_lossy().to_lowercase())
            .unwrap_or_default()
            .chars()
            .map(|c| if valid_name(&c.to_string()) { c } else { '-' })
            .collect();
        let name = if valid_name(&name) {
            name
        } else {
            "app".to_string()
        };
        let mut manifest = Map::new();
        manifest.insert("name".to_string(), json!(name));
        manifest.insert("version".to_string(), json!("0.1.0"));
        manifest.insert("main".to_string(), json!("main.kp"));
        manifest.insert("dependencies".to_string(), json!({}));
        write_json(&cwd.join(MANIFEST), &manifest)?;
        if !cwd.join("main.kp").exists() {
            fs::write(
                cwd.join("main.kp"),
                format!("print(\"hello from {name}\")\n"),
            )
            .map_err(|error| format!("cannot write main.kp: {error}"))?;
        }
        if !cwd.join(".gitignore").exists() {
            let _ = fs::write(cwd.join(".gitignore"), ".khipu/\n");
        }
        println!("created {MANIFEST} for {name}; run it with: khipu run");
        Ok(())
    }

    fn set_dependency(root: &Path, name: &str, source: Option<&str>) -> Result<(), String> {
        let path = root.join(MANIFEST);
        let mut manifest = read_json(&path)?;
        let deps = manifest
            .entry("dependencies")
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .ok_or("dependencies in khipu.json should be an object")?;
        match source {
            Some(source) => {
                deps.insert(name.to_string(), json!(source));
            }
            None => {
                if deps.remove(name).is_none() {
                    return Err(format!("'{name}' is not a dependency"));
                }
            }
        }
        write_json(&path, &manifest)
    }

    pub fn command(args: &[String], cwd: &Path) -> Result<(), String> {
        let words: Vec<&str> = args.iter().map(String::as_str).collect();
        match words.as_slice() {
            ["init"] => init(cwd),
            ["add", name, source] => {
                if !valid_name(name) {
                    return Err(format!(
                        "'{name}' is not a valid package name (use lowercase letters, digits, - and _)"
                    ));
                }
                let root = root(cwd)?;
                let before = fs::read_to_string(root.join(MANIFEST)).unwrap_or_default();
                set_dependency(&root, name, Some(source))?;
                match install(&root) {
                    Ok(installed) => {
                        println!("added {name}");
                        report(&installed);
                        Ok(())
                    }
                    Err(error) => {
                        let _ = fs::write(root.join(MANIFEST), before);
                        Err(error)
                    }
                }
            }
            ["remove", name] => {
                let root = root(cwd)?;
                set_dependency(&root, name, None)?;
                let installed = install(&root)?;
                println!("removed {name}");
                report(&installed);
                Ok(())
            }
            ["install"] => {
                let installed = install(&root(cwd)?)?;
                report(&installed);
                Ok(())
            }
            _ => Err("usage: khipu init | khipu add <name> <git url[#tag] or path:folder> | khipu remove <name> | khipu install".to_string()),
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use std::fs;
    use std::process::Command;

    use super::{command, locate};
    use crate::interpreter::{Interpreter, Value};
    use crate::lexer::lex;
    use crate::parser::Parser;

    fn git(dir: &std::path::Path, args: &[&str]) {
        let status = Command::new("git")
            .args(["-c", "user.name=test", "-c", "user.email=test@example.com"])
            .arg("-C")
            .arg(dir)
            .args(args)
            .status()
            .unwrap();
        assert!(status.success());
    }

    fn run(dir: &std::path::Path, source: &str) -> String {
        let program = Parser::new(lex(source).unwrap()).parse_program().unwrap();
        match Interpreter::in_dir(dir).run_main(&program) {
            Ok(Some(Value::Str(text))) => text.to_string(),
            other => panic!("unexpected {:?}", other.map_err(|e| e.message)),
        }
    }

    #[test]
    fn installs_pins_and_imports_packages() {
        let base = std::env::temp_dir().join(format!("khipu-packages-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let library = base.join("greet");
        let app = base.join("app");
        fs::create_dir_all(&library).unwrap();
        fs::create_dir_all(&app).unwrap();

        git(&library, &["init", "-q"]);
        fs::write(
            library.join("main.kp"),
            "fn hello() -> string {\n    return \"v1\"\n}\n",
        )
        .unwrap();
        git(&library, &["add", "-A"]);
        git(&library, &["commit", "-qm", "one"]);
        git(&library, &["tag", "v1"]);
        fs::write(
            library.join("main.kp"),
            "fn hello() -> string {\n    return \"v2\"\n}\n",
        )
        .unwrap();
        git(&library, &["commit", "-qam", "two"]);

        let words = |list: &[&str]| list.iter().map(|w| w.to_string()).collect::<Vec<_>>();
        command(&words(&["init"]), &app).unwrap();
        let source = format!("{}#v1", library.display());
        command(&words(&["add", "greet", &source]), &app).unwrap();
        let program = "import greet from \"greet\"\ngreet.hello()";
        assert_eq!(run(&app, program), "v1");

        fs::remove_dir_all(app.join(".khipu")).unwrap();
        command(&words(&["install"]), &app).unwrap();
        assert_eq!(run(&app, program), "v1");

        command(
            &words(&["add", "greet", &library.display().to_string()]),
            &app,
        )
        .unwrap();
        assert_eq!(run(&app, program), "v2");

        assert!(command(&words(&["add", "Bad", "x"]), &app).is_err());
        assert!(command(&words(&["add", "evil", "--upload-pack=x"]), &app).is_err());
        assert!(
            locate(&app, "../greet")
                .unwrap_err()
                .contains("not a package name")
        );
        command(&words(&["remove", "greet"]), &app).unwrap();
        assert!(locate(&app, "greet").unwrap_err().contains("not installed"));
        fs::remove_dir_all(&base).unwrap();
    }
}
