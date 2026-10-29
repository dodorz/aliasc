use aliasc::{backend, compile_model, context::{DistroArg, EnvironmentArg, PlatformArg, Shell}, manifest, CompileOptions, Context};
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::{path::PathBuf, process::ExitCode};

#[derive(Parser)] #[command(name="aliasc", version=env!("ALIASC_VERSION"), about="Alias DSL v2 compiler")]
struct Cli { #[command(subcommand)] command: Command }
#[derive(Subcommand)] enum Command { Compile { #[command(flatten)] input: Input, #[arg(long)] output:PathBuf }, Validate { #[command(flatten)] input:Input }, List { #[command(flatten)] input:Input, #[arg(long,value_enum,default_value="name")] format:ListFormat }, Search { #[command(flatten)] input:Input, #[arg(required=true)] pattern:Vec<String> }, Explain { #[command(flatten)] input:Input, name:String } }
#[derive(Args,Clone)] struct Input { #[arg(long,value_enum)] shell:Shell, #[arg(long,default_value="auto",value_enum)] platform:PlatformArg, #[arg(long,default_value="auto")] distro:DistroArg, #[arg(long,default_value="auto")] environment:EnvironmentArg, #[arg(long)] source:PathBuf, #[arg(long)] no_local:bool, #[arg(long,conflicts_with="no_shortcut_map")] shortcut_map:Option<PathBuf>, #[arg(long)] no_shortcut_map:bool }
fn main()->ExitCode { let cli=Cli::parse();let (input,action)=match cli.command {Command::Compile{input,output}=>(input,Action::Compile(output)),Command::Validate{input}=>(input,Action::Validate),Command::List{input,format}=>(input,Action::List(format)),Command::Search{input,pattern}=>(input,Action::Search(pattern)),Command::Explain{input,name}=>(input,Action::Explain(name))};let context=Context::resolve(input.shell,input.platform,input.distro,input.environment);let default_shortcut_map=input.shortcut_map.unwrap_or_else(||aliasc::default_shortcut_map(&input.source));let options=CompileOptions{context,source:input.source,local:!input.no_local,shortcut_map:if input.no_shortcut_map{None}else{Some(default_shortcut_map)}};let model=match compile_model(&options){Ok(x)=>x,Err(d)=>{print_diags(&d);return ExitCode::FAILURE}};print_diags(&model.diagnostics);match action {Action::Validate=>ExitCode::SUCCESS,Action::List(format)=>{for d in &model.definitions{match format{ListFormat::Name=>println!("{}",d.name),ListFormat::Tsv=>println!("{}\t{}",d.name,escape_tsv(&backend::display_body(&d.body)))}}ExitCode::SUCCESS},Action::Search(pattern)=>{let pat=pattern.iter().map(|x|x.to_ascii_lowercase()).collect::<Vec<_>>();for d in &model.definitions{if pat.iter().all(|p|d.name.to_ascii_lowercase().contains(p)){println!("{}\t{}",d.name,d.span)}}ExitCode::SUCCESS},Action::Explain(name)=>{match model.definitions.iter().find(|d|d.name.eq_ignore_ascii_case(&name)){Some(d)=>{println!("name: {}\nsource: {}\nmode: {}",d.name,d.span,if d.legacy{"legacy-cmd"}else{"portable"});ExitCode::SUCCESS},None=>{eprintln!("aliasc: definition `{name}` is not active for this target context");ExitCode::FAILURE}}},Action::Compile(output)=>{let generated=match backend::generate(&model.context,&model.definitions){Ok(x)=>x,Err(d)=>{print_diags(&d);return ExitCode::FAILURE}};match manifest::write_outputs(&output,&model,generated){Ok(())=>ExitCode::SUCCESS,Err(e)=>{eprintln!("aliasc: {}: {e}",output.display());ExitCode::FAILURE}}}} }
enum Action{Compile(PathBuf),Validate,List(ListFormat),Search(Vec<String>),Explain(String)}
#[derive(Clone,Copy,ValueEnum)] enum ListFormat{Name,Tsv}
fn escape_tsv(s:&str)->String{let mut out=String::with_capacity(s.len());for c in s.chars(){match c{'\\'=>out.push_str("\\\\"),'\t'=>out.push_str("\\t"),'\n'=>out.push_str("\\n"),'\r'=>out.push_str("\\r"),_=>out.push(c)}}out}
fn print_diags(diags:&[aliasc::Diagnostic]){
    let mut cache:std::collections::HashMap<std::path::PathBuf,Option<String>>=std::collections::HashMap::new();
    for d in diags{
        eprintln!("{d}");
        let line_text=cache.entry(d.span.file.clone())
            .or_insert_with(||std::fs::read_to_string(&d.span.file).ok())
            .as_ref()
            .and_then(|c|c.lines().nth(d.span.line.saturating_sub(1)));
        if let Some(line)=line_text{
            let num=d.span.line.to_string();
            let pad=" ".repeat(num.len());
            let col=d.span.column.saturating_sub(1);
            eprintln!("{} | {}",num,line);
            eprintln!("{} |{}^",pad," ".repeat(col));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::escape_tsv;

    #[test]
    fn tsv_escapes_backslashes_and_control_characters() {
        assert_eq!(escape_tsv("a\tb\nc\\d\re"), "a\\tb\\nc\\\\d\\re");
    }
}
