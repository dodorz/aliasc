use crate::Context;
use serde::Serialize;
use std::{fmt, path::PathBuf};

#[derive(Clone, Debug, Serialize)]
pub struct SourceSpan { pub file: PathBuf, pub line: usize, pub column: usize }
impl fmt::Display for SourceSpan { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}:{}:{}", self.file.display(), self.line, self.column) } }
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum Severity { Error, Warning }
#[derive(Clone, Debug, Serialize)]
pub struct Diagnostic { pub severity: Severity, pub message: String, pub span: SourceSpan, pub include_stack: Vec<PathBuf> }
impl Diagnostic { pub fn error(span: SourceSpan, message: impl Into<String>, include_stack: Vec<PathBuf>) -> Self { Self { severity: Severity::Error, message: message.into(), span, include_stack } } pub fn warning(span: SourceSpan, message: impl Into<String>, include_stack: Vec<PathBuf>) -> Self { Self { severity: Severity::Warning, message: message.into(), span, include_stack } } }
impl fmt::Display for Diagnostic { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}: {}: {}", self.span, match self.severity { Severity::Error => "error", Severity::Warning => "warning" }, self.message)?; if self.include_stack.len() > 1 { write!(f, "\n  include stack: {}", self.include_stack.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(" -> "))?; } Ok(()) } }

#[derive(Clone, Debug)]
pub struct RawDefinition { pub name: String, pub body: String, pub section: Option<String>, pub span: SourceSpan, pub include_stack: Vec<PathBuf>, pub local: bool }
#[derive(Clone, Debug)]
pub struct Definition { pub name: String, pub template: Template, pub span: SourceSpan, pub legacy: bool, pub context: Context }
#[derive(Clone, Debug)]
pub enum Template { Command(CommandTemplate), SetEnv(Vec<(String, String)>), UnsetEnv(Vec<String>), WithEnv { vars: Vec<(String,String)>, body: Option<Box<CommandTemplate>> }, FirstAvailable(Vec<CommandTemplate>), LegacyCmdTemplate(String) }
#[derive(Clone, Debug)]
pub struct CommandTemplate { pub pipeline: Pipeline, pub implicit_all: bool, pub bypass_shell_function: bool }
#[derive(Clone, Debug)]
pub struct Pipeline { pub commands: Vec<CommandInvocation> }
#[derive(Clone, Debug)]
pub struct CommandInvocation { pub arguments: Vec<Argument>, pub input: Option<Argument>, pub output: Option<(Argument, bool)>, pub bypass_shell_function: bool }
#[derive(Clone, Debug)]
pub struct Argument { pub segments: Vec<ArgumentSegment>, pub quoted: bool, pub all_arguments: bool }
#[derive(Clone, Debug)]
pub enum ArgumentSegment { Literal(String), EnvRef(String), Positional(u8), AllArguments, LiteralAt, CommandSubstitution(Box<CommandTemplate>) }

pub fn valid_env(name: &str) -> bool { let mut c = name.chars(); matches!(c.next(), Some(x) if x.is_ascii_alphabetic() || x == '_') && c.all(|x| x.is_ascii_alphanumeric() || x == '_') }
pub fn valid_name(name: &str) -> bool { !name.is_empty() && !name.contains(char::is_whitespace) && !name.contains('=') }

pub fn parse_template(body: &str, span: &SourceSpan, stack: &[PathBuf]) -> Result<Template, Diagnostic> {
    let b = body.trim();
    if b.starts_with("Shell:") { return Err(err(span, "Shell:<dialect>[...] is not supported by Alias DSL v2", stack)); }
    if has_forbidden_shell_syntax(b) { return Err(err(span, "portable template contains unsupported shell syntax", stack)); }
    if let Some(content) = wrapped(b, "SetEnv") { return parse_assignments(content, span, stack).map(Template::SetEnv); }
    if let Some(content) = wrapped(b, "UnsetEnv") { return parse_names(content, span, stack).map(Template::UnsetEnv); }
    if let Some(rest) = b.strip_prefix("WithEnv(") { let end = matching_paren(rest).ok_or_else(|| err(span, "unclosed WithEnv(...)" ,stack))?; let vars = parse_assignments(&rest[..end], span, stack)?; let tail = rest[end+1..].trim(); let body = if tail.is_empty() { None } else { Some(Box::new(parse_command(tail, span, stack)?)) }; return Ok(Template::WithEnv { vars, body }); }
    if let Some(content) = wrapped(b, "FirstAvailable") { let mut candidates = Vec::new(); for p in split_top(content, ',') { if p.trim().is_empty() { return Err(err(span, "FirstAvailable has an empty candidate", stack)); } candidates.push(parse_command(p.trim(), span, stack)?); } if candidates.is_empty() { return Err(err(span, "FirstAvailable needs a candidate", stack)); } return Ok(Template::FirstAvailable(candidates)); }
    Ok(Template::Command(parse_command(b, span, stack)?))
}
fn has_forbidden_shell_syntax(s: &str) -> bool { let chars: Vec<char> = s.chars().collect(); let mut quote = None; let mut i = 0; while i < chars.len() { let c = chars[i]; if let Some(q) = quote { if q == '"' && c == '\\' && chars.get(i + 1).is_some() { i += 2; continue; } if c == q { quote = None; } else if c == '`' { return true; } i += 1; continue; } match c { '\'' | '"' => quote = Some(c), '`' | ';' => return true, '&' if chars.get(i + 1) == Some(&'&') => return true, '|' if chars.get(i + 1) == Some(&'|') => return true, _ => {} } i += 1; } false }
fn err(span: &SourceSpan, message: impl Into<String>, stack: &[PathBuf]) -> Diagnostic { Diagnostic::error(span.clone(), message, stack.to_vec()) }
fn wrapped<'a>(s: &'a str, name: &str) -> Option<&'a str> { let rest = s.strip_prefix(name)?.strip_prefix('(')?; let end = matching_paren(rest)?; if rest[end+1..].trim().is_empty() { Some(&rest[..end]) } else { None } }
fn matching_paren(s: &str) -> Option<usize> { let mut depth=0usize; let mut quote=None; for (i,c) in s.char_indices() { if let Some(q)=quote { if c==q { quote=None }; continue } match c { '\''|'"'=>quote=Some(c), '('=>depth+=1, ')'=> { if depth==0{return Some(i)}; depth-=1; }, _=>{} } } None }
fn parse_assignments(s: &str, span:&SourceSpan, stack:&[PathBuf])->Result<Vec<(String,String)>,Diagnostic>{ let mut out=Vec::new(); for token in split_words(s, span, stack)? { let Some((name,value))=token.split_once('=') else { return Err(err(span,"expected NAME=value",stack)); }; if !valid_env(name) {return Err(err(span,format!("invalid environment variable name `{name}`"),stack));} out.push((name.to_string(),value.to_string())); } if out.is_empty(){return Err(err(span,"expected at least one environment assignment",stack));} Ok(out) }
fn parse_names(s:&str,span:&SourceSpan,stack:&[PathBuf])->Result<Vec<String>,Diagnostic>{ let names=split_words(s,span,stack)?; if names.is_empty(){return Err(err(span,"expected at least one environment variable name",stack));} for n in &names {if !valid_env(n){return Err(err(span,format!("invalid environment variable name `{n}`"),stack));}} Ok(names) }
fn split_words(s:&str,span:&SourceSpan,stack:&[PathBuf])->Result<Vec<String>,Diagnostic>{ tokenize(s,span,stack).map(|v|v.into_iter().filter(|x|!matches!(x,Tok::Op(_))).map(|x|match x{Tok::Word(w)=>w.raw, _=>unreachable!()}).collect()) }
#[derive(Clone)] struct Word { raw:String, arg:Argument }
#[derive(Clone)] enum Tok { Word(Word), Op(String) }
fn parse_command(s:&str,span:&SourceSpan,stack:&[PathBuf])->Result<CommandTemplate,Diagnostic>{ let tokens=tokenize(s,span,stack)?; let mut pipeline=Pipeline{commands:Vec::new()}; let mut current=CommandInvocation{arguments:Vec::new(),input:None,output:None,bypass_shell_function:false}; let mut need_redirect:Option<String>=None; let mut saw=false; let mut any_placeholder=false;
 for tok in tokens { match tok { Tok::Op(op) if op=="|"=> { if current.arguments.is_empty(){return Err(err(span,"pipeline has an empty command",stack));} pipeline.commands.push(current);current=CommandInvocation{arguments:Vec::new(),input:None,output:None,bypass_shell_function:false}; }, Tok::Op(op) if op=="<"||op==">"||op==">>"=>{if need_redirect.is_some(){return Err(err(span,"redirection is missing a target",stack));}need_redirect=Some(op)}, Tok::Op(op)=>return Err(err(span,format!("unsupported operator `{op}`"),stack)), Tok::Word(w)=>{saw=true; any_placeholder|=contains_placeholder(&w.arg);if let Some(op)=need_redirect.take(){if w.arg.all_arguments{return Err(err(span,"@* cannot be a redirection target",stack));}if op=="<"{if current.input.replace(w.arg).is_some(){return Err(err(span,"more than one input redirection",stack));}}else if current.output.replace((w.arg,op==">>")).is_some(){return Err(err(span,"more than one output redirection",stack));}}else{current.arguments.push(w.arg)}} }}
 if need_redirect.is_some(){return Err(err(span,"redirection is missing a target",stack));}if !saw||current.arguments.is_empty(){return Err(err(span,"expected a command invocation",stack));}pipeline.commands.push(current);for command in &pipeline.commands { if command.arguments.first().is_some_and(is_assignment_word) { return Err(err(span,"name=value command prefixes are not portable Alias DSL syntax",stack)); } }Ok(CommandTemplate{pipeline,implicit_all:!any_placeholder,bypass_shell_function:false}) }
fn is_assignment_word(a:&Argument)->bool { if a.quoted || a.segments.len()!=1 { return false; } match &a.segments[0] { ArgumentSegment::Literal(s)=>s.split_once('=').is_some_and(|(name,_)|valid_env(name)), _=>false } }
fn contains_placeholder(a:&Argument)->bool{a.segments.iter().any(|s|matches!(s,ArgumentSegment::Positional(_)|ArgumentSegment::AllArguments))}
fn tokenize(s:&str,span:&SourceSpan,stack:&[PathBuf])->Result<Vec<Tok>,Diagnostic>{let mut out=Vec::new();let chars:Vec<char>=s.chars().collect();let(mut i,mut buf,mut segs,mut quote,mut quoted)=(0usize,String::new(),Vec::new(),None,false);let flush=|buf:&mut String,segs:&mut Vec<ArgumentSegment>,quoted:bool,out:&mut Vec<Tok>|->Result<(),Diagnostic>{if !buf.is_empty(){segs.push(ArgumentSegment::Literal(std::mem::take(buf)));}if !segs.is_empty(){let all=segs.len()==1&&matches!(segs[0],ArgumentSegment::AllArguments)&&!quoted;if segs.iter().any(|x|matches!(x,ArgumentSegment::AllArguments))&&!all{return Err(err(span,"@* must be an unquoted standalone argument",stack));}let raw=segs.iter().filter_map(|s|match s{ArgumentSegment::Literal(v)=>Some(v.as_str()),ArgumentSegment::LiteralAt=>Some("@"),_=>None}).collect::<String>();out.push(Tok::Word(Word{raw,arg:Argument{segments:std::mem::take(segs),quoted,all_arguments:all}}));}Ok(())};
 while i<chars.len(){let c=chars[i];if let Some(q)=quote{if q=='"'&&c=='\\'&&chars.get(i+1).is_some(){buf.push(chars[i+1]);i+=2;continue} if c==q{quote=None;i+=1;continue} if q=='\''{buf.push(c);i+=1;continue} }else if c=='\''||c=='"'{quote=Some(c);quoted=true;i+=1;continue}else if c.is_whitespace(){flush(&mut buf,&mut segs,quoted,&mut out)?;quoted=false;i+=1;continue}else if c=='|'||c=='<'||c=='>'{flush(&mut buf,&mut segs,quoted,&mut out)?;quoted=false;let op=if c=='>'&&chars.get(i+1)==Some(&'>'){i+=1;">>"}else{match c{'|'=>"|",'<'=>"<",_=>">"}};out.push(Tok::Op(op.into()));i+=1;continue}
 if c=='$'&&chars.get(i+1)==Some(&'{'){let mut j=i+2;while j<chars.len()&&chars[j]!='}'{j+=1}if j==chars.len(){return Err(err(span,"unclosed ${...} environment reference",stack));}if !buf.is_empty(){segs.push(ArgumentSegment::Literal(std::mem::take(&mut buf)));}let name: String=chars[i+2..j].iter().collect();if !valid_env(&name){return Err(err(span,format!("invalid environment variable name `{name}`"),stack));}segs.push(ArgumentSegment::EnvRef(name));i=j+1;continue}
 if c=='$'&&chars.get(i+1)==Some(&'('){let rest:String=chars[i+2..].iter().collect();let Some(end)=matching_paren(&rest) else{return Err(err(span,"unclosed $(...) command substitution",stack))};if !buf.is_empty(){segs.push(ArgumentSegment::Literal(std::mem::take(&mut buf)));}segs.push(ArgumentSegment::CommandSubstitution(Box::new(parse_command(&rest[..end],span,stack)?)));i+=end+3;continue}
 if c=='$'{return Err(err(span,"bare $NAME is not Alias DSL v2 syntax; use ${NAME}",stack));}
 if c=='@'{if !buf.is_empty(){segs.push(ArgumentSegment::Literal(std::mem::take(&mut buf)));}match chars.get(i+1){Some('@')=>{segs.push(ArgumentSegment::LiteralAt);i+=2},Some('*')=>{segs.push(ArgumentSegment::AllArguments);i+=2},Some(n) if n.is_ascii_digit()&&*n!='0'=>{segs.push(ArgumentSegment::Positional(n.to_digit(10).unwrap()as u8));i+=2},_=>{buf.push('@');i+=1}};continue}buf.push(c);i+=1}
 if quote.is_some(){return Err(err(span,"unclosed quoted argument",stack));}flush(&mut buf,&mut segs,quoted,&mut out)?;Ok(out)}
fn split_top(s:&str,sep:char)->Vec<&str>{let mut o=Vec::new();let(mut start,mut depth,mut q)=(0,0usize,None);for(i,c)in s.char_indices(){if let Some(x)=q{if c==x{q=None};continue}match c{'\''|'"'=>q=Some(c),'('=>depth+=1,')'=>depth=depth.saturating_sub(1),_ if c==sep&&depth==0=>{o.push(&s[start..i]);start=i+c.len_utf8()},_=>{}}}o.push(&s[start..]);o}
