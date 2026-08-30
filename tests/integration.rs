use aliasc::{backend, compile_model, context::{Distro, Environment, Platform, Shell}, dsl::{parse_template, SourceSpan}, manifest, CompileOptions, Context};
use std::{fs, path::PathBuf};
use tempfile::tempdir;

fn context(platform: Platform) -> Context { Context { shell: Shell::Posix, platform, distro: Distro::None, environment: Environment::None } }
fn options(source: PathBuf, platform: Platform) -> CompileOptions { CompileOptions { context: context(platform), source, local: true, shortcut_map: None } }

#[test]
fn resolver_expands_relative_duplicate_includes_and_local_override() {
    let d=tempdir().unwrap(); let root=d.path();
    fs::write(root.join("child"), "[Common]\nfrom_child=printf child\n").unwrap();
    fs::write(root.join("alias"), "include \"child\"\ninclude \"child\"\n[Common]\nvalue=printf main\n").unwrap();
    fs::write(root.join("alias.local"), "value=printf local\n").unwrap();
    let result=compile_model(&options(root.join("alias"),Platform::Linux)).unwrap();
    assert_eq!(result.includes.len(),2);
    assert_eq!(result.definitions.iter().map(|x|x.name.as_str()).collect::<Vec<_>>(),vec!["from_child","value"]);
    let generated=backend::generate(&result.context,&result.definitions).unwrap();
    assert!(generated.primary.contains("'local'"));
}

#[test]
fn sections_are_selected_by_platform_not_shell() {
    let d=tempdir().unwrap(); let source=d.path().join("alias");
    fs::write(&source,"[Common]\nvalue=printf common\n[UNIX]\nvalue=printf unix\n[Windows]\nvalue=dir /w\nnative=SetEnv(TEST_VALUE=one)\n[Bash]\nignored=printf no\n").unwrap();
    let linux=compile_model(&options(source.clone(),Platform::Linux)).unwrap();
    assert_eq!(linux.definitions.len(),1);
    let windows=compile_model(&options(source,Platform::Windows)).unwrap();
    assert!(windows.definitions.iter().any(|d|d.name == "value" && d.legacy));
    assert!(windows.definitions.iter().any(|d|d.name == "native" && !d.legacy));
    assert!(windows.diagnostics.iter().any(|x|x.message.contains("shell section")));
}

#[test]
fn portable_parser_rejects_unsafe_syntax_and_bad_all_arguments() {
    let span=SourceSpan{file:PathBuf::from("fixture"),line:1,column:1};
    assert!(parse_template("Shell:cmd[echo hi]",&span,&[]).is_err());
    assert!(parse_template("echo \"@*\"",&span,&[]).is_err());
    assert!(parse_template("A=one echo",&span,&[]).is_err());
    assert!(parse_template("echo $HOME",&span,&[]).is_err());
}

#[test]
fn missing_include_is_warning_and_cycle_is_error() {
    let d=tempdir().unwrap(); let source=d.path().join("alias");
    fs::write(&source,"include \"missing\"\n[Common]\nok=printf ok\n").unwrap();
    let result=compile_model(&options(source.clone(),Platform::Linux)).unwrap();
    assert!(result.diagnostics.iter().any(|x|x.message.contains("does not exist")));
    fs::write(&source,"include \"other\"\n[Common]\nok=printf ok\n").unwrap();
    fs::write(d.path().join("other"),"include \"alias\"\n").unwrap();
    assert!(compile_model(&options(source,Platform::Linux)).is_err());
}

#[test]
fn cmd_runtime_discards_the_macro_name_before_forwarding_arguments() {
    let d=tempdir().unwrap(); let source=d.path().join("alias");
    fs::write(&source,"[Common]\ncat=FirstAvailable(bat, cat)\n[Windows]\npkg=scoop\nrfg=rg @*\n").unwrap();
    let mut o=options(source,Platform::Windows); o.context.shell=Shell::Cmd;
    let model=compile_model(&o).unwrap(); let generated=backend::generate(&model.context,&model.definitions).unwrap();
    assert!(!generated.primary.starts_with("::"));
    let runtime=&generated.sibling.unwrap().1;
    assert!(runtime.contains(":cat\nshift /1\n"));
    assert!(runtime.contains("call :__aliasc_find_external \"bat\" __aliasc_exec_cat"));
    assert!(runtime.contains("call \"%__aliasc_exec_cat%\" %1 %2 %3 %4 %5 %6 %7 %8 %9"));
    assert!(runtime.contains(":pkg\nshift /1\nscoop %1 %2 %3 %4 %5 %6 %7 %8 %9"));
    assert!(runtime.contains(":rfg\nshift /1\nrg %1 %2 %3 %4 %5 %6 %7 %8 %9"));
    assert!(!runtime.contains("\"bat\" %*"));
}

#[test]
fn cmd_runtime_renders_command_substitution_as_a_single_argument() {
    let d=tempdir().unwrap(); let source=d.path().join("alias");
    fs::write(&source,"[Common]\nchoose=outer $(inner first | filter second)\n").unwrap();
    let mut o=options(source,Platform::Windows); o.context.shell=Shell::Cmd;
    let model=compile_model(&o).unwrap(); let generated=backend::generate(&model.context,&model.definitions).unwrap();
    let runtime=&generated.sibling.unwrap().1;
    assert!(runtime.contains("for /f \"delims=\" %%S in ('call \"inner\" \"first\" ^| call \"filter\" \"second\""));
    assert!(runtime.contains("call \"outer\" \"%__aliasc_sub_0%\""));
}

#[test]
fn powershell_omits_empty_forwarded_arguments() {
    let d=tempdir().unwrap(); let source=d.path().join("alias");
    fs::write(&source,"[Common]\nimplicit=echo hello\nexplicit=echo hello @*\n").unwrap();
    let mut o=options(source,Platform::Windows); o.context.shell=Shell::Powershell;
    let model=compile_model(&o).unwrap(); let generated=backend::generate(&model.context,&model.definitions).unwrap();
    assert!(generated.primary.contains("if ($AliasArgs.Count -gt 0) { & (__aliasc_path 'echo') (__aliasc_path 'hello') @AliasArgs } else { & (__aliasc_path 'echo') (__aliasc_path 'hello') }"));
}

#[test]
fn manifest_tracks_missing_optional_inputs_and_all_outputs() {
    let d=tempdir().unwrap(); let source=d.path().join("alias"); let output=d.path().join("aliases.mac");
    fs::write(&source,"[Common]\nx=printf x\n").unwrap();
    let mut o=options(source,Platform::Windows);o.context.shell=Shell::Cmd;
    let model=compile_model(&o).unwrap(); let generated=backend::generate(&model.context,&model.definitions).unwrap();
    manifest::write_outputs(&output,&model,generated).unwrap();
    let text=fs::read_to_string(format!("{}.manifest.json",output.display())).unwrap();
    assert!(text.contains("ShortcutMap.yaml"));
    assert!(text.contains("aliasc-runtime.cmd"));
    assert!(output.exists());
}
