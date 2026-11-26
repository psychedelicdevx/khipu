# Phase 8 audit

The last step of Phase 8: every roadmap item checked against the code and the tests that cover it, the benchmark results after the performance work, and the findings of an independent review of the repository. Dated 2026-11-26, commit at the head of `main` when this file was last touched.

## How this was checked

- `cargo test` runs 111 unit tests, the fuzz smoke run (1 500 mutated programs) and the WebAssembly check (the emitted module runs in `node` and is compared with the interpreter). A 60 000-program fuzz run was done once for the record.
- `scripts/check-docs.py` runs every Khipu snippet in `README.md` and every program in `examples/` as written. The pre-commit hook runs it on every commit.
- `python3 scripts/bench.py` reran the whole suite; the tables in `README.md` and `BENCHMARKS.md` are its output.
- The review in the last section was done by a separate agent with no part in writing the code, reading the repository cold.

## Phase 0: Foundations

| Item | Where | Checked by |
|---|---|---|
| Lexer: numbers and math operators | `src/lexer.rs` | `lexer::tests::lexes_simple_math` |
| Parser with correct precedence | `src/parser.rs` | `interpreter::tests::respects_precedence_and_parentheses` |
| Calculator REPL | `src/main.rs` `repl` | `the_prompt_remembers_types_and_recovers_from_mistakes` (the same `Interpreter::run` the prompt uses) |
| Variables and `print` | `src/compiler.rs`, `src/value.rs` `print` | `runs_a_multi_line_program`, `check-docs.py` |
| Running `.kp` files | `src/main.rs` `run_file` | every example in `check-docs.py` |
| `//` comments | `src/lexer.rs` | `lexer::tests::skips_comments_and_counts_lines` |
| Errors with line numbers | `src/error.rs` | `error::tests`, `reports_every_error_with_its_line` |
| Booleans, comparisons, `&&` `\|\|` `!`, `%` | `src/value.rs` `binary` | `compares_and_combines` |
| `if` / `else if` / `else` | `src/compiler.rs` | `picks_the_right_branch` |
| `while` loops and reassignment | `src/compiler.rs` | `loops_with_while` |
| Block scope | `src/compiler.rs` `begin_scope` | `keeps_block_variables_inside_the_block` |
| No truthiness, no coercion | `src/checker.rs`, `src/vm.rs` `CheckBool` | `has_no_truthiness_or_coercion` |

## Phase 1: Core language

| Item | Where | Checked by |
|---|---|---|
| Strings, `+`, escapes | `src/lexer.rs`, `src/value.rs` | `joins_and_compares_strings`, `lexer::tests::lexes_strings_with_escapes` |
| String interpolation | `src/lexer.rs` templates, `Code::Concat`, `Code::Stringify` | `interpolates_expressions_into_strings`, `lists_of_one_kind_can_still_take_anything` |
| Functions | `src/compiler.rs` `function` | `calls_functions_and_returns_values` |
| Recursion and closures | `src/vm.rs` frames, cells | `supports_recursion`, `closures_remember_their_variables` |
| `print` as a built-in | `src/value.rs` `NATIVES` | `functions_without_return_give_none` |
| Lists | `src/value.rs` `List` | `builds_and_reads_lists`, `changes_lists_in_place` |
| Records | `src/value.rs` `Fields` | `builds_and_reads_records`, `changes_existing_fields` |
| `for`, `break`, `continue` | `Code::IterInit`, `Code::ForNext` | `loops_over_lists_and_strings`, `breaks_and_continues` |
| `none` instead of `null` and `undefined` | `src/value.rs`, `src/checker.rs` `Optional` | `none_is_the_one_empty_value`, `nullable_types_need_a_fallback` |
| Modules | `src/vm.rs` `load_module` | `imports_a_whole_module` and the other `imports_*` tests |

## Phase 2: Speed

| Item | Where | Checked by |
|---|---|---|
| Benchmark suite | `benchmarks/`, `scripts/bench.py` | the tables below |
| Variables resolved to slots | `src/compiler.rs` `resolve` | all interpreter tests |
| Scopes skipped for blocks that declare nothing | `src/compiler.rs` `end_scope` emits `PopN` only when needed | `keeps_block_variables_inside_the_block` |
| Cheap-to-share strings | `Value::Str(Rc<String>)` | `updates_variables_in_place_safely` |
| Bytecode compiler | `src/compiler.rs` | everything |
| Virtual machine | `src/vm.rs` | everything |
| Faster than Python on every benchmark | see the sustained table below | `scripts/bench.py` |

## Phase 3: Types

| Item | Where | Checked by |
|---|---|---|
| Type annotations | `src/parser.rs` types, `src/checker.rs` | `accepts_type_annotations`, `rejects_mismatched_annotations` |
| Type checker before running | `Interpreter::execute` calls `Checker::check` first | `checks_types_before_anything_runs` |
| Inference for `let` | `src/checker.rs` | `infers_types_from_the_first_value` |
| Typed signatures | `src/checker.rs` `check_args` | `checks_function_signatures`, `checks_function_types` |
| Structs with methods | `Code::MakeStructDef`, `Code::Invoke` | `builds_structs_and_calls_methods`, `checks_struct_usage` |
| Enums and `match` | `Code::Test`, `src/checker.rs` match arms | `matches_enums`, `match_needs_every_case`, `matches_plain_values` |
| Generics | `src/checker.rs` `Type::Param`, `substitute` | `checks_generic_lists_and_functions` |

## Phase 4: Benchmarks

| Item | Where | Checked by |
|---|---|---|
| Median of 20+ runs with spread | `scripts/bench.py` (21 cold runs, 5 sustained) | `BENCHMARKS.md` |
| Cold-start and sustained | `benchmarks/suite.json` two sizes | `BENCHMARKS.md` |
| Peak memory | `scripts/bench.py` `run_once` reads `ru_maxrss` | memory table |
| CPU set | `benchmarks/cpu/` | tables |
| Microbenchmarks | `benchmarks/functions/` | tables |
| C and Rust versions | `benchmarks/**/*.c`, `*.rs` | `bench.py` checks all five print the same answer |
| Interleaved runs with raw results | `bench.py` shuffles; `benchmarks/results/` | files |
| BENCHMARKS.md with the losses | `BENCHMARKS.md` | reading it |

## Phase 5: Tooling

| Item | Where | Checked by |
|---|---|---|
| VS Code highlighting | `editors/vscode/syntaxes` | opening a `.kp` file (manual) |
| `.kp` icon | `editors/vscode/icons` | manual |
| Errors that point at the problem | `src/error.rs` `underline` | `error::tests` |
| `khipu fmt` | `src/format.rs` | `format::tests`, fuzz checks formatting is stable, pre-commit checks every `.kp` |
| `khipu test` | `src/main.rs` `test_files`, `Stmt::Test` | `skips_tests_in_a_normal_run_and_asserts`, `examples/testing.kp` in the hook |
| Language server | `src/lsp.rs` | `lsp::tests` |
| Web playground | `playground/`, `src/wasm.rs` | opened in a browser during this audit; `scripts/build-playground.sh` |
| Tree-sitter grammar | `editors/tree-sitter-khipu` | its corpus tests (`npx tree-sitter-cli test`); not rerun in this audit, the grammar did not change in Phase 8 |

## Phase 6: Concurrency

| Item | Where | Checked by |
|---|---|---|
| `spawn` | `src/task.rs` `spawn` | `runs_tasks_that_talk_over_channels` |
| Channels | `src/task.rs` `send`, `receive` | same |
| Every core | `src/task.rs` `workers` starts one thread per core | `parallel` benchmark |
| `select` | `src/parser.rs` `select_statement`, `task::select` | same test, `reports_every_error_with_its_line` |
| No shared mutable state | `task::pack` copies every value sent | the `copies` case in `runs_tasks_that_talk_over_channels` |

## Phase 7: Ecosystem

| Item | Where | Checked by |
|---|---|---|
| Standard library | `src/stdlib.rs`, `src/prelude.kp` | `has_a_standard_library` |
| Files and the terminal | `src/stdlib.rs` file natives | same test (writes, appends, lists, deletes a file) |
| HTTP | `src/http.rs` | `speaks_json_and_http` |
| Contracts | `src/parser.rs` `requires`/`ensures`/`example` desugaring | `checks_contracts`, `examples/contracts.kp` |
| Package manager | `src/package.rs` | `package::tests` |
| Compile to WASM for the web | `src/web.rs`, `src/wasm.rs`, `build.rs` | `web::tests`; a generated page was opened in a browser during this audit |

## Phase 8: Performance and final audit

| Item | Where | Checked by |
|---|---|---|
| Lazy `range()` | `List::Range` in `src/value.rs` | `ranges_count_without_building_a_list` (`len(range(100000000000))` in no time) |
| Garbage collector that frees cycles | `src/gc.rs` | `gc::tests`, including a program leaving 1 000 cycles behind |
| Peak memory below Python and Node | memory table below | `bench.py` |
| Faster function calls | `src/compiler.rs` `fuse`, `src/native.rs` | `fib`, `fib_iterative` rows below |
| Beat Python on every sustained benchmark | sustained table below | `bench.py` |
| Native compilation (Cranelift) | `src/kernel.rs`, `src/jit.rs`, `src/native.rs` | `jit::tests`, `hot_numeric_functions_run_natively_and_fall_back_when_needed`, `hot_loops_run_natively_with_the_same_results` |
| Compile straight to WebAssembly instructions | `src/wasmgen.rs`, `khipu web` | `tests/wasm_kernels.rs` runs the module in `node`; a page was timed in a browser (`fib(32)` 25 ms) |
| Competitive with Node | sustained table below, honestly read | `bench.py` |
| Start-up under 5 ms | `bench.py` start-up line | 3.9 ms median |
| Work stealing | `src/task.rs` `POOL`, idle workers | `parallel` benchmark, `runs_tasks_that_talk_over_channels` |
| Tasks take turns in deep recursion | `src/vm.rs` budget in `enter!` | `tasks_take_turns_inside_deep_recursion` |
| Fuzzing finds no crash | `tests/fuzz.rs` | 60 000 programs clean; 1 500 on every `cargo test` |
| Every error message tested | `reports_every_error_with_its_line`, `package::tests::explains_every_package_mistake`, `web::tests` | the remaining untested strings are `internal error` invariants and playground-only messages, listed below |
| No `unsafe`, or every use justified | three uses, each with a comment | `jit::tests` compare native with interpreted results |
| Every roadmap item re-checked | this file | |
| Full benchmark rerun | `README.md`, `BENCHMARKS.md` | `bench.py` on 2026-11-26 |
| Every example and doc snippet runs | `scripts/check-docs.py` | 26 snippets and examples, in the pre-commit hook |
| Independent review | last section | |

### Error messages without a test

These cannot be reached from a program, so they have no test:

- The `internal error: ...` messages in `src/vm.rs` and `src/compiler.rs` guard invariants the compiler establishes (a field name constant is always a string, a captured variable is always a cell, and so on).
- `only a struct can be built with { ... }` in `src/vm.rs`: the parser only accepts a struct name before `{`.
- The playground-only messages (`... is not available in the playground`) are compiled only for `wasm32`.
- The `needs a string` / `needs a number` checks in `src/stdlib.rs` and `src/http.rs` natives: every native is reached through a typed prelude wrapper, so a wrong type is reported by the method lookup first (`number has no method 'upper'`), which is tested.
- `the server stopped: ...` in `src/http.rs`: needs the listening socket to fail after it was opened.

## Benchmark results

Filled in from the rerun; see `README.md` for the sustained table and `BENCHMARKS.md` for cold starts, spreads and memory.

## Independent review

Filled in below from the review's findings, each one fixed or written down.
