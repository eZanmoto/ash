// Copyright 2025-2026 Sean Kelleher. All rights reserved.
// Use of this source code is governed by an MIT
// licence that can be found in the LICENCE file.

#[cfg(test)]
extern crate assert_matches;
extern crate snafu;

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::Error as IoError;
use std::path::Path;
use std::path::PathBuf;
use std::process;
use std::sync::Arc;
use std::sync::Mutex;

mod ast;
mod builtins;
mod eval;
mod lexer;

use lalrpop_util::ParseError;
use snafu::ResultExt;
use snafu::Snafu;

use ast::RawExpr;
use builtins::fns;
use builtins::type_functions;
use eval::builtins::Builtins;
use eval::EvaluationContext;
use eval::error::Error as EvalError;
use eval::value;
use eval::value::Object;
use eval::value::SourcedValue;
use eval::value::Value;
use eval::scope::ScopeStack;
use lexer::Lexer;
use lexer::LexError;
use lexer::Token;
use parser::ProgParser;

#[macro_use]
extern crate lalrpop_util;

lalrpop_mod!(
    #[allow(clippy::all)]
    #[allow(clippy::pedantic)]
    #[allow(dead_code)]
    #[allow(unused_imports)]
    parser
);

fn main() {
    let mut args = std::env::args();
    let prog =
        match args.next() {
            Some(v) => v,
            None => {
                eprintln!("couldn't get program name");
                process::exit(101);
            },
        };

    let raw_cur_rel_script_path =
        match args.next() {
            Some(v) => v,
            None => {
                eprintln!("usage: {prog} <script-path>");
                process::exit(102);
            },
        };

    let cur_rel_script_path = Path::new(&raw_cur_rel_script_path);

    if let Err(e) = run(cur_rel_script_path) {
        let msg =
            match e {
                Error::GetCurrentDirFailed{source} => {
                    format!(" couldn't get current directory: {source}")
                },
                Error::ReadScriptFailed{path, source} => {
                    let p = path.to_string_lossy();

                    format!(" couldn't read script at '{p}': {source}")
                },
                Error::ParseFailed{src} => {
                    let ((ln, ch), msg) = render_parse_error(src);

                    format!("{ln}:{ch}: {msg}")
                },
                Error::EvalFailed{source, path} => {
                    let e = eval::root_error(&source);
                    if let EvalError::Runtime{err_obj} = e {
                        let v = render_error_object(
                            cur_rel_script_path,
                            err_obj,
                        );
                        match v {
                            Ok(v) =>
                                v.to_string(),
                            Err(e) =>
                                format!(
                                    // TODO Add the rendered error object.
                                    "couldn't render error object: {e}",
                                ),
                        }
                    } else {
                        let st = eval_err_to_stacktrace(&path, None, source);

                        let mut rendered_stacktrace = String::new();
                        if !st.stacktrace.is_empty() {
                            rendered_stacktrace = format!(
                                "\nStacktrace:\n  {}",
                                st.stacktrace.join("\n  "),
                            );
                        }

                        format!("{}{}", st.msg, rendered_stacktrace)
                    }
                },
            };
        eprintln!("{raw_cur_rel_script_path}:{msg}");
        process::exit(103);
    }
}

fn run(cur_rel_script_path: &Path) -> Result<(), Error> {
    let cur_script_dir = env::current_dir()
        .context(GetCurrentDirFailed)?;
    let mut cur_script_path = cur_script_dir.clone();
    cur_script_path.push(cur_rel_script_path);

    let src = fs::read_to_string(&cur_script_path)
        .context(ReadScriptFailed{path: cur_script_path.clone()})?;

    let global_bindings = vec![
        (
            RawExpr::Var{name: "print".to_string()},
            value::new_built_in_func("print".to_string(), fns::print),
        ),
    ];

    let mut scopes = ScopeStack::new(vec![]);
    let lexer = Lexer::new(&src);
    let ast =
        match ProgParser::new().parse(lexer) {
            Ok(v) => {
                v
            },
            Err(e) => {
                return Err(Error::ParseFailed{src: e});
            },
        };

    eval::eval_prog(
        &EvaluationContext{
            builtins: &Builtins{
                std: Arc::new(Mutex::new(BTreeMap::new())),
                type_functions: type_functions::type_functions(),
            },
            cur_script_dir,
            cur_func: None,
        },
        &mut scopes,
        global_bindings.clone(),
        &ast,
    )
        .context(EvalFailed{path: cur_rel_script_path})?;

    Ok(())
}

#[derive(Debug, Snafu)]
#[snafu(context(suffix(false)))]
#[allow(clippy::enum_variant_names)]
enum Error {
    GetCurrentDirFailed{source: IoError},
    ReadScriptFailed{path: PathBuf, source: IoError},
    // We add `ParseError` as a `src` value rather than `source` because it
    // doesn't satisfy the error constraints required by `Snafu`.
    ParseFailed{src: ParseError<(usize, usize), Token, LexError>},
    EvalFailed{source: EvalError, path: PathBuf},
}

fn render_parse_error(error: ParseError<(usize, usize), Token, LexError>)
    -> ((usize, usize), String)
{
    match error {
        ParseError::InvalidToken{location} => {
            (location, "invalid token".to_string())
        },
        ParseError::UnrecognizedEof{location, expected} =>
            (
                location,
                format!(
                    "unexpected EOF; expected {}",
                    join_strings(&expected),
                ),
            ),
        ParseError::UnrecognizedToken{token: (loc, tok, _loc), expected} =>
            (
                loc,
                format!(
                    "unexpected '{}'; expected {}",
                    render_token(tok),
                    join_strings(&expected),
                ),
            ),
        ParseError::ExtraToken{token: (loc, tok, _loc)} =>
            (loc, format!("encountered extra token '{tok:?}'")),
        ParseError::User{error} =>
            match error {
                LexError::Unexpected(loc, c) =>
                    (loc, format!("unexpected '{c}'")),
                LexError::IntOverflow(loc, raw_int) =>
                    (loc, format!("'{raw_int}' is too high for an int")),
                LexError::InvalidEscapeChar(loc, c) =>
                    (loc, format!("'{c}' is not a valid escape character")),
                LexError::InvalidHexChar(loc, c) =>
                    (loc, format!("'{c}' is not a valid hex character")),
                LexError::UnescapedDollar(loc) =>
                    (loc, "'$' must be escaped".to_string()),
                LexError::InvalidInterpolationStart(loc, c) =>
                    (
                        loc,
                        format!(
                            "interpolation slots start with '{{', got '{c}'",
                        ),
                    ),
            },
    }
}

fn render_token(t: Token) -> String {
    match t {
        Token::Ident(s) => format!("`{s}`"),
        Token::IntLiteral(n) => format!("{n}"),

        Token::StrLiteral(s)
        | Token::InterpStrLiteral(s, _) => format!("\"{s}\""),

        Token::Break => "`break`".to_string(),
        Token::Continue => "`continue`".to_string(),
        Token::Else => "`else`".to_string(),
        Token::False => "`false`".to_string(),
        Token::Fn => "`fn`".to_string(),
        Token::For => "`for`".to_string(),
        Token::If => "`if`".to_string(),
        Token::In => "`in`".to_string(),
        Token::Null => "`null`".to_string(),
        Token::Return => "`return`".to_string(),
        Token::Throw => "`throw`".to_string(),
        Token::True => "`true`".to_string(),
        Token::While => "`while`".to_string(),

        Token::Bang => "!".to_string(),
        Token::BraceClose => "}".to_string(),
        Token::BraceOpen => "{".to_string(),
        Token::BracketClose => "]".to_string(),
        Token::BracketOpen => "[".to_string(),
        Token::Colon => ":".to_string(),
        Token::Comma => ",".to_string(),
        Token::Div => "/".to_string(),
        Token::Dot => ".".to_string(),
        Token::Equals => "=".to_string(),
        Token::GreaterThan => ">".to_string(),
        Token::LessThan => "<".to_string(),
        Token::Mod => "%".to_string(),
        Token::Mul => "*".to_string(),
        Token::ParenClose => ")".to_string(),
        Token::ParenOpen => "(".to_string(),
        Token::StmtEnd => "stmt_end".to_string(),
        Token::Question => "?".to_string(),
        Token::Sub => "-".to_string(),
        Token::Sum => "+".to_string(),

        Token::AmpAmp => "&&".to_string(),
        Token::BangEquals => "!=".to_string(),
        Token::ColonColon => "::".to_string(),
        Token::ColonEquals => ":=".to_string(),
        Token::DashGreaterThan => "->".to_string(),
        Token::DivEquals => "/=".to_string(),
        Token::DollarBraceOpen => "${".to_string(),
        Token::DollarBracketOpen => "$[".to_string(),
        Token::DotDot => "..".to_string(),
        Token::EqualsEquals => "==".to_string(),
        Token::GreaterThanEquals => ">=".to_string(),
        Token::LessThanEquals => "<=".to_string(),
        Token::ModEquals => "%=".to_string(),
        Token::MulEquals => "*=".to_string(),
        Token::PipePipe => "||".to_string(),
        Token::QuestionQuestion => "??".to_string(),
        Token::SubEquals => "-=".to_string(),
        Token::SumEquals => "+=".to_string(),

        Token::BangEqualsEquals => "!==".to_string(),
        Token::DollarColonEquals => "$:=".to_string(),
        Token::EqualsEqualsEquals => "===".to_string(),
    }
}

fn join_strings(xs: &[String]) -> String {
    if xs.is_empty() {
        String::new()
    } else if xs.len() == 1 {
        xs[0].clone()
    } else {
        let pre = xs[0 .. xs.len() - 1].join(", ");
        let last = xs[xs.len() - 1].clone();

        format!("{pre} or {last}")
    }
}

#[allow(clippy::too_many_lines)]
fn eval_err_to_stacktrace(path: &Path, func: Option<&str>, error: EvalError)
    -> StacktracedErrorMsg
{
    match error {
        EvalError::BindFailed{source} |
        EvalError::BindObjectCollectFailed{source} |
        EvalError::BindObjectSingleFailed{source} |
        EvalError::BindObjectPairFailed{source} |
        EvalError::BindListItemFailed{source} |
        EvalError::BindNextFailed{source} |
        EvalError::EvalProgFailed{source} |
        EvalError::EvalStmtsInNewScopeFailed{source} |
        EvalError::EvalStmtsWithScopeStackFailed{source} |
        EvalError::EvalStmtsFailed{source} |
        EvalError::EvalDeclarationRhsFailed{source} |
        EvalError::DeclarationBindFailed{source} |
        EvalError::EvalAssignmentRhsFailed{source} |
        EvalError::AssignmentBindFailed{source} |
        EvalError::OpAssignmentBindFailed{source} |
        EvalError::EvalIfConditionFailed{source} |
        EvalError::EvalIfStatementsFailed{source} |
        EvalError::EvalElseStatementsFailed{source} |
        EvalError::EvalWhileConditionFailed{source} |
        EvalError::EvalWhileStatementsFailed{source} |
        EvalError::EvalForIterFailed{source} |
        EvalError::ConvertForIterToPairsFailed{source} |
        EvalError::EvalForStatementsFailed{source} |
        EvalError::ValidateArgsFailed{source} |
        EvalError::DeclareFunctionFailed{source} |
        EvalError::EvalThrowExprFailed{source} |
        EvalError::EvalBlockFailed{source} |
        EvalError::EvalStmtFailed{source} |
        EvalError::EvalBinOpLhsFailed{source} |
        EvalError::EvalBinOpRhsFailed{source} |
        EvalError::ApplyBoolOpFailed{source} |
        EvalError::ApplyBinOpFailed{source} |
        EvalError::BinOpAssignListIndexFailed{source} |
        EvalError::EvalReturnExprFailed{source} |
        EvalError::BinOpAssignObjectIndexFailed{source} |
        EvalError::BinOpAssignPropFailed{source} |
        EvalError::EvalListItemsFailed{source} |
        EvalError::EvalListItemFailed{source} |
        EvalError::EvalSourceExprFailed{source} |
        EvalError::EvalStringIndexFailed{source} |
        EvalError::EvalListIndexFailed{source} |
        EvalError::EvalObjectIndexFailed{source} |
        EvalError::EvalObjectPropFailed{source} |
        EvalError::EvalIndexToI64Failed{source} |
        EvalError::EvalStartIndexFailed{source} |
        EvalError::EvalEndIndexFailed{source} |
        EvalError::EvalStringRangeIndexFailed{source} |
        EvalError::EvalListRangeIndexFailed{source} |
        EvalError::EvalRangeStartFailed{source} |
        EvalError::EvalRangeEndFailed{source} |
        EvalError::EvalObjectPropsFailed{source} |
        EvalError::EvalPropNameFailed{source} |
        EvalError::EvalPropValueFailed{source, ..} |
        EvalError::EvalCallFailed{source} |
        EvalError::EvalCallArgsFailed{source} |
        EvalError::InsertStackFrameFailed{source} |
        EvalError::EvalCallFuncFailed{source} |
        EvalError::EvalErrorObjectMsgFailed{source} |
        EvalError::EvalErrorObjectContextFailed{source} |
        EvalError::EvalErrorObjectSourcesFailed{source} |
        EvalError::EvalCatchAsBoolFailed{source} |
        EvalError::EvalCatchAsErrorFailed{source} |
        EvalError::EvalExprFailed{source} |
        EvalError::EvalPropFailed{source} |
        EvalError::InterpolateStringFailed{source} |
        EvalError::InterpolateStringEvalExprFailed{source} |
        EvalError::AssertArgsFailed{source} |
        EvalError::AssertThisFailed{source} |
        EvalError::AssertNoThisFailed{source} |
        EvalError::AssertStrFailed{source} => {
            eval_err_to_stacktrace(path, func, *source)
        },

        EvalError::EvalBuiltinFuncCallFailed{source, func_name, call_loc} => {
            let next_func =
                func_name.unwrap_or_else(|| "<unnamed function>".to_string());
            let mut st =
                eval_err_to_stacktrace(path, Some(&next_func), *source);
            let (line, col) = call_loc;
            let sep =
                if let Some(f) = func {
                    format!(" in '{f}':")
                } else {
                    String::new()
                };

            st.msg = format!("{}:{}:{} {}", line, col, sep, st.msg);

            st
        },

        EvalError::EvalFuncCallFailed{source, func_name, call_loc} => {
            let next_func =
                func_name.unwrap_or_else(|| "<unnamed function>".to_string());
            let mut st =
                eval_err_to_stacktrace(path, Some(&next_func), *source);
            let p = path.to_string_lossy();
            let (line, col) = call_loc;
            let f = func.unwrap_or("<root>");

            st.stacktrace.push(format!("{p}:{line}:{col}: in '{f}'"));

            st
        },

        EvalError::AtLoc{source, line, col} => {
            let mut st = eval_err_to_stacktrace(path, func, *source);
            let sep =
                if let Some(f) = func {
                    format!(" in '{f}':")
                } else {
                    String::new()
                };

            st.msg = format!("{}:{}:{} {}", line, col, sep, st.msg);

            st
        },

        EvalError::Runtime{err_obj} => {
            let msg = format!("{err_obj:?}");

            StacktracedErrorMsg{stacktrace: vec![], msg}
        },

        _ => {
            StacktracedErrorMsg{stacktrace: vec![], msg: format!("{error}")}
        },
    }
}

struct StacktracedErrorMsg {
    stacktrace: Vec<String>,
    msg: String,
}

fn render_error_object(file_path: &Path, err_obj: &Object)
    -> Result<String, String>
{
    let mut render = String::new();

    let SourcedValue{v: msg_value, ..} =
        if let Some(v) = err_obj.get("msg") {
            v
        } else {
            return Err("error object doesn't contain 'msg'".to_string())
        };

    let msg =
        if let Value::Str(s) = msg_value {
            s
        } else {
            return Err("error object 'msg' isn't 'string'".to_string())
        };

    match String::from_utf8(msg.clone()) {
        Ok(n) => {
            render += &format!("{n}\n");
        },
        Err(_) => {
            return Err(format!("invalid UTF-8 for message {msg:?}"));
        },
    }

    let SourcedValue{v: stack_value, ..} =
        if let Some(v) = err_obj.get("stack") {
            v
        } else {
            return Err("error object doesn't contain 'stack'".to_string());
        };

    let items =
        if let Value::List{items, ..} = stack_value {
            items
        } else {
            return Err("error object 'stack' isn't 'list'".to_string());
        };

    let items_val = &lock_deref!(items);

    render += "Stacktrace:";

    for item in items_val {
        let s = render_error_object_stack_frame(file_path, item)?;
        render += &s;
    }

    if let Some(v) = err_obj.get("sources") {
        let SourcedValue{v: sources_value, ..} = v;

        let sources =
            if let Value::List{items, ..} = sources_value {
                items
            } else {
                return Err("error object 'sources' isn't 'list'".to_string());
            };

        let source_vals = &lock_deref!(sources);

        for source_val in source_vals {
            let err_obj =
                if let Value::Object{props, ..} = &source_val.v {
                    props
                } else {
                    return Err(
                        "error object source isn't 'object'".to_string(),
                    );
                };

            let err_obj_val = &lock_deref!(err_obj);

            let s = render_error_object(file_path, err_obj_val)?;

            render += "\n\nCaused by: ";
            render += &s;
        }
    }

    Ok(render)
}

fn render_error_object_stack_frame(
    file_path: &Path,
    stack_frame: &SourcedValue
)
    -> Result<String, String>
{
    let frame_ref =
        if let Value::Object{props, ..} = &stack_frame.v {
            props
        } else {
            return Err("stack frame isn't 'object'".to_string());
        };

    let frame = &lock_deref!(frame_ref);

    let line_val =
        if let Some(v) = frame.get("line") {
            v
        } else {
            return Err("stack frame doesn't contain 'line'".to_string());
        };

    let line =
        if let Value::Int(n) = line_val.v {
            n
        } else {
            return Err("stack frame 'line' isn't 'int'".to_string());
        };

    let col_val =
        if let Some(v) = frame.get("col") {
            v
        } else {
            return Err("stack frame doesn't contain 'column'".to_string());
        };

    let col =
        if let Value::Int(n) = col_val.v {
            n
        } else {
            return Err("stack frame 'col' isn't 'int'".to_string());
        };

    let mut func_name = "<root>".to_string();
    if let Some(v) = frame.get("fn") {
        match &v.v {
            // TODO Handle `BuiltinFunc`.
            Value::Func(func_ref) => {
                let func = &lock_deref!(func_ref);

                if let Some(n) = &func.name {
                    func_name = n.to_string();
                } else {
                    func_name = "<anonymous function>".to_string();
                }
            },
            _ => {
                return Err("stack frame 'fn' isn't 'fn'".to_string());
            },
        };
    }

    Ok(format!(
        "\n  {}:{}:{}: in '{}'",
        file_path.display(),
        line,
        col,
        func_name,
    ))
}
