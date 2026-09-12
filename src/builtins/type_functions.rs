// Copyright 2025-2026 Sean Kelleher. All rights reserved.
// Use of this source code is governed by an MIT
// licence that can be found in the LICENCE file.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::iter::FromIterator;
use std::sync::Arc;
use std::sync::Mutex;

use snafu::ResultExt;
use snafu::Snafu;

use super::fns;
use crate::eval::builtins::TypeFunctions;
use crate::eval::error::AssertArgsFailed;
use crate::eval::error::AssertStrFailed;
use crate::eval::error::AssertThisFailed;
use crate::eval::error::CastFailed;
use crate::eval::error::Result as EvalResult;
use crate::eval::value;
use crate::eval::value::ObjectRef;
use crate::eval::value::SourcedValue;
use crate::eval::value::Value;

pub fn append(
    type_funcs: &mut TypeFunctions,
    new_type_funcs: HashMap<String, SourcedValue>,
) -> Result<(), Error> {
    for (name, tf) in new_type_funcs {
        if let Some(suffix) = name.strip_prefix("objects_") {
            let mut objects = type_funcs.objects.try_lock().unwrap();

            objects.insert(suffix.to_string(), tf);
        } else {
            return Err(Error::NoSupportedTypeFuncPrefixFound{
                type_func_name: name,
            });
        }
    }

    Ok(())
}

#[derive(Debug, Snafu)]
#[snafu(context(suffix(false)))]
pub enum Error {
    #[snafu(display(
        "no supported type function prefix found on {}",
        type_func_name,
    ))]
    NoSupportedTypeFuncPrefixFound{type_func_name: String},
}

pub fn type_functions() -> TypeFunctions {
    TypeFunctions{
        bools: new_func_map(vec![
            (
                "type".to_string(),
                value::new_built_in_func("bool_type".to_string(), any_type),
            ),
        ]),
        ints: new_func_map(vec![
            (
                "type".to_string(),
                value::new_built_in_func("int_type".to_string(), any_type),
            ),
        ]),
        strs: new_func_map(vec![
            (
                "len".to_string(),
                value::new_built_in_func("str_len".to_string(), str_len),
            ),
            (
                "type".to_string(),
                value::new_built_in_func("str_type".to_string(), any_type),
            ),
        ]),
        lists: new_func_map(vec![
            (
                "type".to_string(),
                value::new_built_in_func("list_type".to_string(), any_type),
            ),
        ]),
        objects: new_func_map(vec![
            (
                "type".to_string(),
                value::new_built_in_func("object_type".to_string(), any_type),
            ),
        ]),
        funcs: new_func_map(vec![
            (
                "type".to_string(),
                value::new_built_in_func("fn_type".to_string(), any_type),
            ),
        ]),
    }
}

pub fn new_func_map(funcs: Vec<(String, SourcedValue)>) -> ObjectRef {
    Arc::new(Mutex::new(BTreeMap::<String, SourcedValue>::from_iter(
        funcs,
    )))
}

#[allow(clippy::needless_pass_by_value)]
pub fn str_len(this: Option<SourcedValue>, vs: Vec<SourcedValue>)
    -> EvalResult<SourcedValue>
{
    fns::assert_args("len", 0, &vs)
        .context(AssertArgsFailed)?;

    let this = fns::assert_this(this)
        .context(AssertThisFailed)?;

    let s = fns::assert_str("this", &this)
        .context(AssertStrFailed)?;

    let n: i64 = s.len().try_into()
        .context(CastFailed)?;

    Ok(value::new_int(n))
}

#[allow(clippy::needless_pass_by_value)]
pub fn any_type(this: Option<SourcedValue>, vs: Vec<SourcedValue>)
    -> EvalResult<SourcedValue>
{
    fns::assert_args("type", 0, &vs)
        .context(AssertArgsFailed)?;

    let this = fns::assert_this(this)
        .context(AssertThisFailed)?;

    let s = render_type(&this.v);

    Ok(value::new_str_from_string(s))
}

// TODO Duplicated from `src/eval/error.rs`.
fn render_type(v: &Value) -> String {
    let s =
        match v {
            Value::Null => "null",

            Value::Bool(_) => "bool",
            Value::Int(_) => "int",
            Value::Str(_) => "string",

            Value::List{..} => "list",
            Value::Object{..} => "object",

            Value::BuiltinFunc{..} | Value::Func{..} => "func",
        };

    s.to_string()
}
