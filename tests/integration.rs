use aliasc::{backend, compile_model, context::{Distro, Environment, Platform, Shell}, dsl::{parse_template, SourceSpan, Template}, manifest, CompileOptions, Context};
use std::{fs, path::PathBuf};
use tempfile::tempdir;

fn context(platform: Platform) -> Context { Context { shell: Shell::Posix, platform, distro: Distro::None, environment: Environment::None } }
fn options(source: PathBuf, platform: Platform) -> CompileOptions { CompileOptions { context: context(platform), source, local: true, shortcut_map: None } }

#[test]
fn definition_names_are_case_insensitive_and_last_definition_wins() {
    let d=tempdir().unwrap(); let source=d.path().join("alias");
    fs::write(&source,"[Common]\nPiWeb=printf first\npiweb=printf second\n").unwrap();
    let model=compile_model(&options(source,Platform::Linux)).unwrap();
    assert_eq!(model.definitions.len(),1);
    assert_eq!(model.definitions[0].name,"piweb");
    let generated=backend::generate(&model.context,&model.definitions).unwrap();
    assert!(generated.primary.contains("piweb() {"));
    assert!(generated.primary.contains("Piweb() {"));
    assert!(generated.primary.contains("PIWEB() {"));
    assert!(generated.primary.contains("'second'"));
}

#[cfg(unix)]
#[test]
fn posix_alias_name_variants_execute_the_same_definition() {
    use std::process::Command;
    let d=tempdir().unwrap(); let root=d.path(); let source=root.join("alias"); let output=root.join("aliases.sh");
    fs::write(&source,"[Common]\nPiWeb=printf '%s\\n' ok\n").unwrap();
    let model=compile_model(&options(source,Platform::Linux)).unwrap(); let generated=backend::generate(&model.context,&model.definitions).unwrap(); fs::write(&output,&generated.primary).unwrap();
    let result=Command::new("bash").arg("-c").arg(". \"$1\"; piweb; PIWEB").arg("bash").arg(&output).output().unwrap();
    assert!(result.status.success(),"{}",String::from_utf8_lossy(&result.stderr));
    assert_eq!(String::from_utf8_lossy(&result.stdout),"ok\nok\n");
}
#[test]
fn first_available_same_named_candidate_is_case_insensitive() {
    let d=tempdir().unwrap(); let source=d.path().join("alias");
    fs::write(&source,"[Common]\nPiWeb=FirstAvailable(piweb, fallback)\n").unwrap();
    let model=compile_model(&options(source,Platform::Linux)).unwrap();
    let definition=&model.definitions[0];
    let Template::FirstAvailable(candidates)=&definition.template else { panic!("expected FirstAvailable") };
    assert!(candidates[0].bypass_shell_function);
}
#[test]
fn local_binary_version_uses_the_package_version_fallback() {
    let output=std::process::Command::new(env!("CARGO_BIN_EXE_aliasc"))
        .arg("--version")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(),format!("aliasc {}",env!("CARGO_PKG_VERSION")));
}
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
    let parsed=parse_template("\\foo",&span,&[]).unwrap();
    let Template::Command(command)=parsed else { panic!("expected command template") };
    assert!(matches!(&command.pipeline.commands[0].arguments[0].segments[..], [aliasc::dsl::ArgumentSegment::Literal(value)] if value=="\\foo"));
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
fn unix_backend_bypasses_same_named_first_available_command_only() {
    let d=tempdir().unwrap(); let source=d.path().join("alias");
    fs::write(&source,"[Common]\nls=FirstAvailable(ls, dir /w)\ndir=dir --color=auto\n").unwrap();
    let model=compile_model(&options(source,Platform::Linux)).unwrap();
    let generated=backend::generate(&model.context,&model.definitions).unwrap();
    assert!(generated.primary.contains("if __aliasc_is_external 'ls'; then __aliasc_first_ls=0"));
    assert!(generated.primary.contains("0) command 'ls' \"$@\""));
    assert!(generated.primary.contains("dir() {\n  command 'dir' '--color=auto' \"$@\""));
}
#[test]
fn ordinary_aliases_and_first_available_candidates_keep_normal_rendering() {
    let d=tempdir().unwrap(); let source=d.path().join("alias");
    fs::write(&source,"[Common]\nplain=echo hello\nchooser=FirstAvailable(foo-helper, bar)\n").unwrap();
    let model=compile_model(&options(source,Platform::Linux)).unwrap(); let generated=backend::generate(&model.context,&model.definitions).unwrap();
    assert!(generated.primary.contains("plain() {\n  'echo' 'hello' \"$@\""));
    assert!(generated.primary.contains("if __aliasc_is_external 'foo-helper'; then __aliasc_first_chooser=0"));
    assert!(generated.primary.contains("0) 'foo-helper' \"$@\""));
    assert!(!generated.primary.contains("0) command 'foo-helper'"));
}

#[test]
fn first_available_marks_only_an_exact_same_named_command() {
    let d=tempdir().unwrap(); let source=d.path().join("alias");
    fs::write(&source,"[Common]\nfoo=FirstAvailable(foo, foo-helper, env MODE=1 foo, /path/to/foo, bar)\nother=FirstAvailable(foo, bar)\n").unwrap();
    let model=compile_model(&options(source,Platform::Linux)).unwrap();
    let foo=model.definitions.iter().find(|x|x.name=="foo").unwrap();
    let Template::FirstAvailable(candidates)=&foo.template else { panic!("expected FirstAvailable") };
    assert_eq!(candidates.iter().map(|x|x.bypass_shell_function).collect::<Vec<_>>(),vec![true,false,false,false,false]);
    let other=model.definitions.iter().find(|x|x.name=="other").unwrap();
    let Template::FirstAvailable(candidates)=&other.template else { panic!("expected FirstAvailable") };
    assert!(!candidates[0].bypass_shell_function);
}

#[test]
fn first_available_uses_native_external_invocation_for_each_shell() {
    let d=tempdir().unwrap(); let source=d.path().join("alias");
    fs::write(&source,"[Common]\nfoo=FirstAvailable(foo --flag, bar)\n").unwrap();
    for (shell,platform) in [(Shell::Posix,Platform::Linux),(Shell::Bash,Platform::Linux),(Shell::Zsh,Platform::Linux),(Shell::Fish,Platform::Linux),(Shell::Nu,Platform::Linux),(Shell::Powershell,Platform::Windows),(Shell::Pwsh,Platform::Windows),(Shell::Cmd,Platform::Windows)] {
        let mut o=options(source.clone(),platform); o.context.shell=shell;
        let model=compile_model(&o).unwrap(); let generated=backend::generate(&model.context,&model.definitions).unwrap();
        let text=generated.sibling.map(|(_,body)|format!("{}\n{}",generated.primary,body)).unwrap_or(generated.primary);
        match shell {
            Shell::Posix|Shell::Bash|Shell::Zsh => { assert!(text.contains("if __aliasc_is_external 'foo'; then")); assert!(text.contains("command 'foo' '--flag' \"$@\"")); assert!(!text.contains("command 'bar'")); }
            Shell::Fish => { assert!(text.contains("if command -sq 'foo'")); assert!(text.contains("command 'foo' '--flag' $argv")); assert!(!text.contains("command 'bar'")); }
            Shell::Nu => { assert!(text.contains("which 'foo' | where type == external")); assert!(text.contains("^foo '--flag' ...$alias_args")); assert!(text.contains("def --env foo")); }
            Shell::Powershell|Shell::Pwsh => { assert!(text.contains("Get-Command (__aliasc_path 'foo') -CommandType Application")); assert!(text.contains("& ((Get-Command (__aliasc_path 'foo') -CommandType Application -ErrorAction Stop).Path)")); assert!(!text.contains("Get-Command (__aliasc_path 'bar') -CommandType Application -ErrorAction Stop")); }
            Shell::Cmd => { assert!(text.contains("call :__aliasc_find_external \"foo\" __aliasc_exec_foo")); assert!(text.contains("call \"%__aliasc_exec_foo%\" \"--flag\" %1 %2 %3 %4 %5 %6 %7 %8 %9")); }
        }
    }
}

#[test]
fn first_available_keeps_selection_cached_and_falls_back_when_same_named_command_is_missing() {
    let d=tempdir().unwrap(); let source=d.path().join("alias");
    fs::write(&source,"[Common]\nfoo=FirstAvailable(foo, bar)\n").unwrap();
    let model=compile_model(&options(source,Platform::Linux)).unwrap(); let generated=backend::generate(&model.context,&model.definitions).unwrap();
    assert!(generated.primary.contains("if [ \"${__aliasc_first_foo+x}\" != x ]; then"));
    assert!(generated.primary.contains("elif __aliasc_is_external 'bar'; then __aliasc_first_foo=1"));
    assert!(generated.primary.contains("0) command 'foo' \"$@\""));
    assert!(generated.primary.contains("1) 'bar' \"$@\""));
    assert!(!generated.primary.contains("0) 'foo' \"$@\""));
}

#[cfg(unix)]
#[test]
fn posix_first_available_executes_external_same_named_command_without_recursion() {
    use std::{os::unix::fs::PermissionsExt, process::Command};
    let d=tempdir().unwrap(); let root=d.path(); let source=root.join("alias"); let output=root.join("aliases.sh");
    fs::write(root.join("foo"),"#!/bin/sh\nprintf 'external foo %s\\n' \"$1\"\n").unwrap();
    fs::set_permissions(root.join("foo"),fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(&source,"[Common]\nfoo=FirstAvailable(foo, bar)\n").unwrap();
    let model=compile_model(&options(source,Platform::Linux)).unwrap(); let generated=backend::generate(&model.context,&model.definitions).unwrap(); fs::write(&output,&generated.primary).unwrap();
    let old_path=std::env::var("PATH").unwrap_or_default(); let result=Command::new("bash").arg("-c").arg(". \"$1\"; foo --flag").arg("bash").arg(&output).env("PATH",format!("{}:{}",root.display(),old_path)).output().unwrap();
    assert!(result.status.success(),"{}",String::from_utf8_lossy(&result.stderr));
    assert_eq!(String::from_utf8_lossy(&result.stdout),"external foo --flag\n");
    fs::remove_file(root.join("foo")).unwrap();
    fs::write(root.join("bar"),"#!/bin/sh\nprintf 'fallback bar %s\\n' \"$1\"\n").unwrap();
    fs::set_permissions(root.join("bar"),fs::Permissions::from_mode(0o755)).unwrap();
    let result=Command::new("bash").arg("-c").arg(". \"$1\"; foo --flag").arg("bash").arg(&output).env("PATH",format!("{}:{}",root.display(),old_path)).output().unwrap();
    assert!(result.status.success(),"{}",String::from_utf8_lossy(&result.stderr));
    assert_eq!(String::from_utf8_lossy(&result.stdout),"fallback bar --flag\n");
}
#[cfg(unix)]
#[test]
fn posix_ordinary_same_named_command_does_not_recurse() {
    use std::{os::unix::fs::PermissionsExt, process::Command};
    let d=tempdir().unwrap(); let root=d.path(); let source=root.join("alias"); let output=root.join("aliases.sh");
    fs::write(root.join("dir"),"#!/bin/sh\nprintf 'external dir %s\\n' \"$1\"\n").unwrap();
    fs::set_permissions(root.join("dir"),fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(&source,"[Common]\ndir=dir --color=auto\n").unwrap();
    let model=compile_model(&options(source,Platform::Linux)).unwrap(); let generated=backend::generate(&model.context,&model.definitions).unwrap(); fs::write(&output,&generated.primary).unwrap();
    let old_path=std::env::var("PATH").unwrap_or_default(); let result=Command::new("bash").arg("-c").arg(". \"$1\"; dir --flag").arg("bash").arg(&output).env("PATH",format!("{}:{}",root.display(),old_path)).output().unwrap();
    assert!(result.status.success(),"{}",String::from_utf8_lossy(&result.stderr));
    assert_eq!(String::from_utf8_lossy(&result.stdout),"external dir --color=auto\n");
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
