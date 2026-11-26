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
        let failed = |error: std::io::Error| format!("cannot write {}: {error}", path.display());
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |time| time.as_nanos());
        let name = path
            .file_name()
            .map_or_else(Default::default, |name| name.to_string_lossy());
        let fresh = path.with_file_name(format!(".{name}.{}.{stamp}.tmp", std::process::id()));
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&fresh)
            .map_err(failed)?;
        let written = std::io::Write::write_all(&mut file, text.as_bytes())
            .and_then(|_| fs::rename(&fresh, path));
        if let Err(error) = written {
            let _ = fs::remove_file(&fresh);
            return Err(failed(error));
        }
        Ok(())
    }

    fn store(root: &Path) -> Result<PathBuf, String> {
        let mut folder = root.to_path_buf();
        for part in [".khipu", "packages"] {
            folder.push(part);
            match fs::symlink_metadata(&folder) {
                Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => {
                    return Err(format!(
                        "{} must be a real folder, not a link or a file; remove it and run khipu install",
                        folder.display()
                    ));
                }
                Ok(_) => {}
                Err(_) => fs::create_dir(&folder)
                    .map_err(|error| format!("cannot create {}: {error}", folder.display()))?,
            }
        }
        Ok(folder)
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
        let missing = || {
            format!("package '{name}' is not installed; add it with: khipu add {name} <git url>")
        };
        let start = from.canonicalize().unwrap_or_else(|_| from.to_path_buf());
        let project = start
            .ancestors()
            .find(|dir| {
                let in_store = dir.parent().is_some_and(|parent| parent.ends_with(STORE));
                dir.join(MANIFEST).is_file() && !in_store
            })
            .ok_or_else(missing)?;
        for dir in start.ancestors() {
            let package = dir.join(STORE).join(name);
            if package.exists() {
                return entry(&package);
            }
            if dir == project {
                break;
            }
        }
        Err(missing())
    }

    fn remote(source: &str) -> bool {
        ["https://", "ssh://", "git@"]
            .iter()
            .any(|scheme| source.starts_with(scheme))
    }

    fn commit_id(commit: &str) -> bool {
        matches!(commit.len(), 40 | 64) && commit.chars().all(|c| c.is_ascii_hexdigit())
    }

    pub fn main_file(cwd: &Path) -> Result<PathBuf, String> {
        entry(&root(cwd)?)
    }

    fn git(args: &[&str], dir: Option<&Path>) -> Result<String, String> {
        let mut command = Command::new("git");
        command.args(["-c", "protocol.ext.allow=never"]);
        for (name, _) in std::env::vars_os() {
            if name.to_string_lossy().starts_with("GIT_") {
                command.env_remove(name);
            }
        }
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
        let folder = store(root)?.join(name);
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
        if folder
            .symlink_metadata()
            .is_ok_and(|meta| meta.file_type().is_symlink())
        {
            remove(&folder)?;
        }
        if folder.join(".git").is_dir() {
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
        let mut wanted: Vec<(String, String, Option<String>)> = dependencies(&manifest)
            .into_iter()
            .map(|(name, source)| (name, source, None))
            .collect();
        let mut installed: BTreeMap<String, (String, String)> = BTreeMap::new();
        while let Some((name, source, parent)) = wanted.pop() {
            if let Some(parent) = &parent
                && !remote(&source)
            {
                return Err(format!(
                    "{parent} wants {name} from '{source}'; a package's own dependencies must come from https://, ssh:// or git@ sources"
                ));
            }
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
                .and_then(|entry| entry.get("commit").and_then(Json::as_str))
                .filter(|commit| *commit != "path");
            if let Some(commit) = pinned
                && !commit_id(commit)
            {
                return Err(format!(
                    "{LOCK} has an invalid commit for {name}: '{commit}'"
                ));
            }
            let commit = fetch(root, &name, &source, pinned)?;
            if let Ok(inner) = read_json(&store(root)?.join(&name).join(MANIFEST)) {
                wanted.extend(
                    dependencies(&inner)
                        .into_iter()
                        .map(|(dependency, source)| (dependency, source, Some(name.clone()))),
                );
            }
            installed.insert(name, (source, commit));
        }

        let store = store(root)?;
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
                if let Ok(mut lock) = read_json(&root.join(LOCK))
                    && lock.remove(*name).is_some()
                {
                    write_json(&root.join(LOCK), &lock)?;
                }
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
        let mut command = Command::new("git");
        for (name, _) in std::env::vars_os() {
            if name.to_string_lossy().starts_with("GIT_") {
                command.env_remove(name);
            }
        }
        let status = command
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

        let precious = base.join("precious");
        fs::create_dir_all(&precious).unwrap();
        fs::write(precious.join("keep.txt"), "mine").unwrap();
        fs::remove_dir_all(app.join(".khipu")).unwrap();
        fs::create_dir_all(app.join(".khipu")).unwrap();
        std::os::unix::fs::symlink(&precious, app.join(".khipu/packages")).unwrap();
        let refused = command(&words(&["install"]), &app).unwrap_err();
        assert!(refused.contains("must be a real folder"));
        assert!(precious.join("keep.txt").exists());
        fs::remove_file(app.join(".khipu/packages")).unwrap();
        command(&words(&["install"]), &app).unwrap();

        fs::remove_file(app.join("khipu.lock")).unwrap();
        let outside = base.join("outside.txt");
        fs::write(&outside, "untouched").unwrap();
        std::os::unix::fs::symlink(&outside, app.join("khipu.lock")).unwrap();
        command(&words(&["install"]), &app).unwrap();
        assert_eq!(fs::read_to_string(&outside).unwrap(), "untouched");
        std::os::unix::fs::symlink(&outside, app.join("khipu.json.tmp")).unwrap();
        std::os::unix::fs::symlink(&outside, app.join(".khipu.lock.tmp")).unwrap();
        command(&words(&["install"]), &app).unwrap();
        assert_eq!(fs::read_to_string(&outside).unwrap(), "untouched");
        assert!(
            !fs::symlink_metadata(app.join("khipu.lock"))
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert!(command(&words(&["add", "evil", "--upload-pack=x"]), &app).is_err());
        assert!(
            locate(&app, "../greet")
                .unwrap_err()
                .contains("not a package name")
        );
        let planted = base.join(".khipu/packages/evil");
        fs::create_dir_all(&planted).unwrap();
        fs::write(planted.join("main.kp"), "let x = 1\n").unwrap();
        assert!(locate(&app, "evil").unwrap_err().contains("not installed"));

        let lock = fs::read_to_string(app.join("khipu.lock")).unwrap();
        let commit = lock
            .split('"')
            .find(|part| part.len() == 40)
            .unwrap()
            .to_string();
        fs::write(app.join("khipu.lock"), lock.replace(&commit, "--orphan=x")).unwrap();
        assert!(
            command(&words(&["install"]), &app)
                .unwrap_err()
                .contains("invalid commit")
        );
        fs::write(app.join("khipu.lock"), lock).unwrap();

        fs::write(
            library.join("khipu.json"),
            "{ \"name\": \"greet\", \"dependencies\": { \"secrets\": \"path:/etc\" } }\n",
        )
        .unwrap();
        git(&library, &["add", "-A"]);
        git(&library, &["commit", "-qm", "three"]);
        let sneaky = command(
            &words(&["add", "greet", &library.display().to_string()]),
            &app,
        );
        assert!(sneaky.unwrap_err().contains("must come from https://"));

        command(&words(&["remove", "greet"]), &app).unwrap();
        assert!(locate(&app, "greet").unwrap_err().contains("not installed"));
        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn explains_every_package_mistake() {
        let base =
            std::env::temp_dir().join(format!("khipu-package-errors-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let app = base.join("app");
        fs::create_dir_all(&app).unwrap();
        let words = |list: &[&str]| list.iter().map(|w| w.to_string()).collect::<Vec<_>>();
        let fails = |list: &[&str]| command(&words(list), &app).unwrap_err();

        assert_eq!(
            fails(&["install"]),
            "no khipu.json here or in any parent folder; start one with: khipu init"
        );
        assert_eq!(
            fails(&["frobnicate"]),
            "usage: khipu init | khipu add <name> <git url[#tag] or path:folder> | khipu remove <name> | khipu install"
        );
        command(&words(&["init"]), &app).unwrap();
        assert_eq!(fails(&["init"]), "khipu.json already exists here");
        assert_eq!(
            fails(&["add", "Bad", "x"]),
            "'Bad' is not a valid package name (use lowercase letters, digits, - and _)"
        );
        assert_eq!(
            fails(&["add", "evil", "--upload-pack=x"]),
            "evil: '--upload-pack=x' is not a valid source"
        );
        assert_eq!(
            fails(&["add", "lost", "path:nowhere"]),
            "lost: cannot find the folder nowhere"
        );
        assert!(
            fails(&["add", "web", "https://127.0.0.1:1/nothing.git"]).starts_with("git clone: ")
        );
        assert_eq!(fails(&["remove", "ghost"]), "'ghost' is not a dependency");
        assert_eq!(
            locate(&app, "../x").unwrap_err(),
            "'../x' is not a package name; to import a file, give its path with .kp at the end"
        );

        let manifest = app.join("khipu.json");
        let shown = app.canonicalize().unwrap().join("khipu.json");
        fs::write(&manifest, "[1]").unwrap();
        assert_eq!(
            fails(&["install"]),
            format!("{} should hold a JSON object", shown.display())
        );
        fs::write(&manifest, "{oops").unwrap();
        assert!(
            fails(&["install"]).starts_with(&format!("{} is not valid JSON: ", shown.display()))
        );

        fs::write(
            &manifest,
            "{ \"dependencies\": { \"Bad Name\": \"path:.\" } }",
        )
        .unwrap();
        assert_eq!(
            fails(&["install"]),
            "'Bad Name' is not a valid package name (use lowercase letters, digits, - and _)"
        );

        let one = base.join("one");
        let two = base.join("two");
        fs::create_dir_all(&one).unwrap();
        fs::create_dir_all(&two).unwrap();
        fs::write(one.join("main.kp"), "let a = 1\n").unwrap();
        fs::write(two.join("main.kp"), "let b = 1\n").unwrap();
        fs::write(
            one.join("khipu.json"),
            "{ \"dependencies\": { \"shared\": \"https://example.invalid/b.git\" } }",
        )
        .unwrap();
        fs::write(
            &manifest,
            "{ \"dependencies\": { \"one\": \"path:../one\", \"shared\": \"https://example.invalid/a.git\" } }",
        )
        .unwrap();
        let conflict = fails(&["install"]);
        assert!(
            conflict
                == "two packages need 'shared' from different places: https://example.invalid/a.git and https://example.invalid/b.git"
                || conflict.starts_with("git clone: "),
            "{conflict}"
        );

        fs::write(&manifest, "{ \"main\": \"../one/main.kp\" }").unwrap();
        assert_eq!(
            super::main_file(&app).unwrap_err(),
            format!(
                "{}: main file ../one/main.kp is outside the package",
                app.canonicalize().unwrap().display()
            )
        );
        fs::write(&manifest, "{ \"main\": \"missing.kp\" }").unwrap();
        assert_eq!(
            super::main_file(&app).unwrap_err(),
            format!(
                "{} has no missing.kp",
                app.canonicalize().unwrap().display()
            )
        );
        fs::remove_dir_all(&base).unwrap();
    }
}
