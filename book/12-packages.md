# 12. Packages

A package is a folder of Khipu code with a `main.kp`. Packages come straight from git, with no central registry, and a project lists what it uses in `khipu.json`.

## Start a project

```bash
mkdir shop
cd shop
khipu init
```

`khipu init` creates two files. `main.kp` is a one-line program, and `khipu.json` describes the project:

```text
{
  "dependencies": {},
  "main": "main.kp",
  "name": "shop",
  "version": "0.1.0"
}
```

It also writes a `.gitignore` that excludes `.khipu/`, where installed packages live. `khipu run` runs the project's main file:

```bash
khipu run
```

```text
hello from shop
```

## Add a package

Give `khipu add` a name and a source. The source is a git URL, optionally followed by `#` and a tag, branch or commit, or `path:` and a folder, which is handy while two projects are developed side by side:

```bash
khipu add colors https://github.com/someone/colors.git#v1.0
khipu add util path:../util
```

The dependency is added to `khipu.json`, the code is installed into `.khipu/packages`, and the exact version is written to `khipu.lock`:

```text
{
  "util": {
    "commit": "path",
    "source": "path:../util"
  }
}
```

For a git source the `commit` is the exact commit that was installed. Commit `khipu.json` and `khipu.lock` to your repository. Anyone who clones the project runs `khipu install`, and gets the same code you have.

## Use a package

Import a package by name, with no `.kp`:

```js
import colors from "colors"

print(colors.red("hello"))
```

```text
<red>hello</red>
```

An import without `.kp` is a package, and with `.kp` it is a file next to yours (chapter 8). The import gives you what the package's `main.kp` defines. The sample above runs against a tiny stand-in package called `colors` that the checker for this book installs, since the real one does not exist.

A package's own dependencies are installed too, but only from `https://`, `ssh://` or `git@` sources, so a package can never make Khipu pull in a folder from your machine. Installing never runs a package's code, and package imports only look inside your own project.

## Exercises

1. Create a folder `greeter` with a `main.kp` that defines `fn hello(name)`, and a second project that adds it with `khipu add greeter path:../greeter`.
2. After `khipu add`, which two files changed, and which one do you commit?
3. A teammate clones your project. What single command gives them the same package versions you used?

### Solutions

In `greeter/main.kp`:

```js
fn hello(name: string) -> string {
    return "hello, ${name}"
}

print(hello("package"))
```

In the other project, after `khipu add greeter path:../greeter`, `main.kp` becomes:

```text
import greeter from "greeter"

print(greeter.hello("Ada"))
```

Both `khipu.json` and `khipu.lock` change (`.khipu/packages` gets the code, but is not committed); you commit the two files. A teammate runs `khipu install`.
