// Copyright 2025 Sean Kelleher. All rights reserved.
// Use of this source code is governed by an MIT
// licence that can be found in the LICENCE file.

use std::collections::BTreeMap;
use std::collections::HashSet;

use snafu::ResultExt;

#[allow(clippy::wildcard_imports)]
use crate::ast::*;
use crate::eval;
use eval::EvaluationContext;
#[allow(clippy::wildcard_imports)]
use super::error::*;
use super::error::Error;
use super::scope;
use super::scope::ScopeStack;
use crate::lock_deref;
use crate::value;
use value::ListRef;
use value::ObjectRef;
use value::SourcedValue;
use value::Value;

// TODO Mostly duplicated from `src/eval/mod.rs`.
macro_rules! match_eval_expr {
    (
        ( $context:ident, $scopes:ident, $expr:expr )
        { $( $key:pat => $value:expr , )* }
    ) => {{
        let value = eval::eval_expr($context, $scopes, $expr)
            .context(EvalExprFailed)?;
        match value.v {
            $( $key => $value , )*
        }
    }};
}

// `bind` associates the values on `rhs` with the names deconstructed from
// `lhs`.
pub fn bind(
    context: &EvaluationContext,
    scopes: &mut ScopeStack,
    lhs: &Expr,
    rhs: SourcedValue,
    bind_type: BindType,
)
    -> Result<()>
{
    bind_next(context, scopes, &mut HashSet::new(), lhs, rhs, None, bind_type)
}

#[derive(Clone, Copy)]
pub enum BindType {
    Declaration,
    Assignment,
}

// `bind_next` performs a bind, but returns an error if a name that's in
// `names_in_binding` gets reused.
#[allow(clippy::too_many_lines)]
pub fn bind_next(
    context: &EvaluationContext,
    scopes: &mut ScopeStack,
    names_in_binding: &mut HashSet<String>,
    lhs: &Expr,
    rhs: SourcedValue,
    op: Option<(BinaryOp, Location)>,
    bind_type: BindType,
)
    -> Result<()>
{
    let (raw_lhs, loc) = lhs;
    let new_loc_err = |source| {
        let (line, col) = loc;

        Err(Error::AtLoc{source: Box::new(source), line: *line, col: *col})
    };
    let new_invalid_bind_error = |s: &str| {
        new_loc_err(Error::InvalidBindTarget{descr: s.to_string()})
    };

    match raw_lhs {
        RawExpr::Var{name} => {
            bind_next_name(
                scopes,
                names_in_binding,
                name,
                loc,
                rhs,
                op,
                bind_type,
            )
        },

        RawExpr::Index{expr, location: locat} => {
            match_eval_expr!((context, scopes, expr) {
                Value::List(items) => {
                    let n = eval::eval_expr_to_index(context, scopes, locat)
                        .context(EvalListIndexFailed)?;

                    if n >= lock_deref!(items).len() {
                        return new_loc_err(Error::Runtime{msg: format!(
                            "index '{n}' is outside the list bounds",
                        )});
                    }

                    let lhs_val = &mut lock_deref!(items)[n as usize];

                    binary_operation_assign(lhs_val, rhs, op)
                        .context(BinOpAssignListIndexFailed)?;

                    Ok(())
                },

                Value::Object(props) => {
                    // TODO Consider whether non-UTF-8 strings can be used to
                    // perform key lookups on objects.
                    let descr = "property";
                    let name =
                        eval::eval_expr_to_str(context, scopes, descr, locat)
                            .context(EvalObjectIndexFailed)?;

                    if let Some(slot) = lock_deref!(props).get_mut(&name) {
                        binary_operation_assign(slot, rhs, op)
                            .context(BinOpAssignObjectIndexFailed)?;

                        return Ok(());
                    }

                    if op.is_some() {
                        return new_loc_err(Error::OpOnUndefinedIndex{name});
                    }

                    lock_deref!(props).insert(name, rhs);

                    Ok(())
                },

                _ => {
                    new_loc_err(Error::ValueNotIndexAssignable)
                },
            })
        },

        RawExpr::RangeIndex{expr, start, end} => {
            if op.is_some() {
                return new_loc_err(Error::OpOnRangeIndex);
            }

            match_eval_expr!((context, scopes, expr) {
                Value::List(mut lhs_items) => {
                    match rhs.v {
                        Value::List(rhs_items) => {
                            let rhs = lock_deref!(rhs_items).clone();

                            bind_range_index(
                                context,
                                scopes,
                                (&mut lhs_items, start, end),
                                loc,
                                &rhs,
                            )
                        },

                        Value::Str(s) => {
                            let chars: Vec<SourcedValue> =
                                s.iter()
                                    .map(|c| value::new_str(vec![*c]))
                                    .collect();

                            bind_range_index(
                                context,
                                scopes,
                                (&mut lhs_items, start, end),
                                loc,
                                &chars,
                            )
                        },

                        value => {
                            new_loc_err(Error::RangeIndexAssignOnNonIndexable{
                                value,
                            })
                        },
                    }
                },

                _ => {
                    // TODO Consider adding the value to the error.
                    new_loc_err(Error::ValueNotRangeIndexAssignable)
                },
            })
        },

        RawExpr::Prop{expr, name, type_prop} => {
            if *type_prop {
                return new_loc_err(Error::AssignToTypeProp)
            }

            match_eval_expr!((context, scopes, expr) {
                Value::Object(props) => {
                    if let Some(slot) = lock_deref!(props).get_mut(name) {
                        binary_operation_assign(slot, rhs, op)
                            .context(BinOpAssignPropFailed)?;

                        return Ok(());
                    }

                    let name = name.clone();

                    if op.is_some() {
                        return new_loc_err(Error::OpOnUndefinedProp{name});
                    }

                    lock_deref!(props).insert(name, rhs);

                    Ok(())
                },

                value => {
                    new_loc_err(Error::PropAccessOnNonObject{value})
                },
            })
        },

        RawExpr::Object{props: lhs_props} => {
            if op.is_some() {
                return new_loc_err(Error::OpOnObjectDestructure);
            }

            match rhs.v {
                Value::Object(rhs_props) => {
                    bind_object(
                        context,
                        scopes,
                        names_in_binding,
                        lhs_props,
                        &rhs_props,
                        bind_type,
                    )
                },

                value => {
                    new_loc_err(Error::ObjectDestructureOnNonObject{value})
                },
            }
        },

        RawExpr::List{items: lhs_items, collect} => {
            if op.is_some() {
                return new_loc_err(Error::OpOnListDestructure);
            }

            match rhs.v {
                Value::List(rhs_items) => {
                    bind_list(
                        context,
                        scopes,
                        names_in_binding,
                        (lhs_items, collect),
                        loc,
                        &rhs_items,
                        bind_type,
                    )
                },

                value => {
                    new_loc_err(Error::ListDestructureOnNonList{value})
                },
            }
        },

        RawExpr::Null =>
            new_invalid_bind_error("`null`"),
        RawExpr::Bool{..} =>
            new_invalid_bind_error("a boolean literal"),
        RawExpr::Int{..} =>
            new_invalid_bind_error("an integer literal"),
        RawExpr::Str{..} =>
            new_invalid_bind_error("a string literal"),
        RawExpr::UnaryOp{..} =>
            new_invalid_bind_error("a unary operation"),
        RawExpr::BinaryOp{..} =>
            new_invalid_bind_error("a binary operation"),
        RawExpr::Range{..} =>
            new_invalid_bind_error("a range operation"),
        RawExpr::Func{..} =>
            new_invalid_bind_error("an anonymous function"),
        RawExpr::Call{..} =>
            new_invalid_bind_error("a function call"),
        RawExpr::CatchAsBool{..} =>
            new_invalid_bind_error("a boolean catch"),
    }
}

pub fn binary_operation_assign(
    lhs: &mut SourcedValue,
    rhs: SourcedValue,
    op: Option<(BinaryOp, Location)>,
) -> Result<()> {
    let mut rhs = rhs;
    if let Some((op, op_loc)) = op {
        let v =
            eval::apply_binary_operation(
                &op,
                &op_loc,
                &lhs.v,
                &rhs.v,
            )
                .context(ApplyBinOpFailed)?;

        rhs = value::new_val_ref_with_no_source(v);
    }

    scope::set(lhs, rhs);

    Ok(())
}

pub fn bind_name(
    scopes: &mut ScopeStack,
    name: &str,
    name_loc: &(usize, usize),
    rhs: SourcedValue,
    bind_type: BindType,
)
    -> Result<()>
{
    let names = &mut HashSet::new();

    bind_next_name(scopes, names, name, name_loc, rhs, None, bind_type)
}

fn bind_next_name(
    scopes: &mut ScopeStack,
    names_in_binding: &mut HashSet<String>,
    name: &str,
    name_loc: &(usize, usize),
    rhs: SourcedValue,
    op: Option<(BinaryOp, Location)>,
    bind_type: BindType,
)
    -> Result<()>
{
    if name == "_" {
        return Ok(())
    }

    let (line, col) = name_loc;
    let new_loc_error = |source| {
        Err(Error::AtLoc{source: Box::new(source), line: *line, col: *col})
    };

    if names_in_binding.contains(name) {
        return new_loc_error(Error::AlreadyInBinding{name: name.to_string()});
    }
    names_in_binding.insert(name.to_string());

    match bind_type {
        BindType::Declaration => {
            if op.is_some() {
                return new_loc_error(Error::Dev{
                    msg: "operation-assignment on declaration".to_string(),
                });
            }

            if let Err((line, col)) = scopes.declare(name, *name_loc, rhs) {
                return new_loc_error(Error::AlreadyInScope{
                    name: name.to_string(),
                    prev_line: line,
                    prev_col: col,
                });
            }
        },
        BindType::Assignment => {
            let mut rhs_val = rhs;
            if let Some((op, op_loc)) = op {
                let lhs_val =
                    if let Some(v) = scopes.get(&name.to_string()) {
                        v
                    } else {
                        return new_loc_error(Error::Undefined{
                            name: name.to_string(),
                        });
                    };

                let raw_v =
                    eval::apply_binary_operation(
                        &op,
                        &op_loc,
                        &lhs_val.v,
                        &rhs_val.v,
                    )
                        .context(ApplyBinOpFailed)?;

                rhs_val = value::new_val_ref_with_no_source(raw_v);
            }

            if !scopes.assign(name, rhs_val) {
                return new_loc_error(Error::Undefined{
                    name: name.to_string(),
                });
            }
        },
    };

    Ok(())
}

fn bind_range_index(
    context: &EvaluationContext,
    scopes: &mut ScopeStack,
    lhs: (&mut ListRef, &Option<Box<Expr>>, &Option<Box<Expr>>),
    lhs_loc: &(usize, usize),
    rhs_items: &[SourcedValue],
)
    -> Result<()>
{
    let new_loc_err = |source| {
        let (line, col) = lhs_loc;

        Err(Error::AtLoc{source: Box::new(source), line: *line, col: *col})
    };

    let (lhs_items, maybe_start, maybe_end) = lhs;

    let start =
        if let Some(start) = maybe_start {
            eval::eval_expr_to_index(context, scopes, start)
                    .context(EvalStartIndexFailed)?
        } else {
            0
        };

    let rhs_len = rhs_items.len();
    let end =
        if let Some(end) = maybe_end {
            eval::eval_expr_to_index(context, scopes, end)
                    .context(EvalEndIndexFailed)?
        } else {
            rhs_len
        };

    let list_len = lock_deref!(lhs_items).len();
    if start > list_len {
        return new_loc_err(Error::RangeStartOutOfListBounds{start, list_len});
    } else if start >= end {
        return new_loc_err(Error::RangeStartNotBeforeEnd{start, end});
    } else if end > list_len {
        return new_loc_err(Error::RangeEndOutOfListBounds{end, list_len});
    }

    let range_len = end - start;
    if range_len != rhs_len {
        return new_loc_err(Error::RangeIndexItemMismatch{
            range_len,
            rhs_len,
        });
    }

    for (i, v) in rhs_items.iter().enumerate() {
        let slot = &mut lock_deref!(lhs_items)[(start+i) as usize];

        *slot = v.clone();
    }

    Ok(())
}

fn bind_object(
    context: &EvaluationContext,
    scopes: &mut ScopeStack,
    names_in_binding: &mut HashSet<String>,
    lhs: &Vec<PropItem>,
    rhs: &ObjectRef,
    bind_type: BindType,
)
    -> Result<()>
{
    // In contrast with lists, we don't explicitly require that the number of
    // elements in the source object is equal to the number of elements in the
    // target object.

    let mut remaining_keys =
        lock_deref!(rhs)
            .keys()
            .cloned()
            .collect::<HashSet<String>>();

    let mut i = 0;
    for prop_item in lhs {
        match prop_item {
            PropItem::Single{expr, is_spread, collect} => {
                let (raw_expr, prop_name_loc) = expr;
                let new_loc_err = |source| {
                    let (line, col) = prop_name_loc;

                    Err(Error::AtLoc{
                        source: Box::new(source),
                        line: *line,
                        col: *col,
                    })
                };

                if *is_spread {
                    return new_loc_err(Error::SpreadOnObjectDestructure);
                }

                let prop_name =
                    if let RawExpr::Var{name} = &raw_expr {
                        name.clone()
                    } else {
                        return new_loc_err(Error::ObjectPropShorthandNotVar);
                    };

                if *collect {
                    if i != lhs.len()-1 {
                        return new_loc_err(Error::ObjectCollectIsNotLast);
                    }

                    let new_rhs: BTreeMap<String, SourcedValue> =
                        remaining_keys
                            .iter()
                            .map(|k| (
                                k.clone(),
                                lock_deref!(rhs)[k].clone(),
                            ))
                            .collect();

                    bind_next_name(
                        scopes,
                        names_in_binding,
                        &prop_name,
                        prop_name_loc,
                        value::new_object(new_rhs),
                        None,
                        bind_type,
                    )
                        .context(BindObjectCollectFailed)?;

                    continue;
                }

                bind_object_prop(
                    context,
                    scopes,
                    names_in_binding,
                    expr,
                    rhs,
                    (&prop_name, prop_name_loc),
                    bind_type,
                )
                    .context(BindObjectSingleFailed)?;

                remaining_keys.remove(&prop_name);
            },

            PropItem::Pair{name, value: new_lhs} => {
                let (_, prop_name_loc) = name;

                let prop_name =
                    eval::eval_expr_to_str(context, scopes, "property", name)
                        .context(EvalObjectIndexFailed)?;

                bind_object_prop(
                    context,
                    scopes,
                    names_in_binding,
                    new_lhs,
                    rhs,
                    (&prop_name, prop_name_loc),
                    bind_type,
                )
                    .context(BindObjectPairFailed)?;

                remaining_keys.remove(&prop_name);
            },
        }

        i += 1;
    }

    Ok(())
}

fn bind_object_prop(
    context: &EvaluationContext,
    scopes: &mut ScopeStack,
    names_in_binding: &mut HashSet<String>,
    lhs: &Expr,
    rhs: &ObjectRef,
    prop_name: (&str, &(usize, usize)),
    bind_type: BindType,
)
    -> Result<()>
{
    if prop_name.0 == "_" {
        return Ok(());
    }

    let new_loc_err = |source| {
        let (line, col) = prop_name.1;

        Err(Error::AtLoc{source: Box::new(source), line: *line, col: *col})
    };

    let new_rhs =
        match lock_deref!(rhs).get(prop_name.0) {
            Some(v) => v.clone(),
            None => return new_loc_err(Error::Runtime{
                msg: format!(
                    "object doesn't contain property '{}'",
                    prop_name.0,
                ),
            }),
        };

    bind_next(context, scopes, names_in_binding, lhs, new_rhs, None, bind_type)
        .context(BindNextFailed)?;

    Ok(())
}

fn bind_list(
    context: &EvaluationContext,
    scopes: &mut ScopeStack,
    names_in_binding: &mut HashSet<String>,
    raw_lhs: (&[ListItem], &bool),
    lhs_loc: &(usize, usize),
    rhs: &ListRef,
    bind_type: BindType,
)
    -> Result<()>
{
    let (lhs, collect) = raw_lhs;

    let new_loc_err = |source| {
        let (line, col) = lhs_loc;

        Err(Error::AtLoc{source: Box::new(source), line: *line, col: *col})
    };

    let lhs_len = lhs.len();
    let rhs_len = lock_deref!(rhs).len();
    if *collect {
        if lhs_len-1 > rhs_len {
            return new_loc_err(Error::ListCollectTooFew{lhs_len, rhs_len});
        }
    } else if lhs_len != rhs_len {
        return new_loc_err(Error::ListDestructureItemMismatch{
            lhs_len,
            rhs_len,
        });
    }

    for i in 0 .. lhs_len {
        let ListItem{expr: lhs, is_spread} = &lhs[i];
        if *is_spread {
            return new_loc_err(Error::SpreadInListDestructure{index: i});
        }

        let rhs =
            if *collect && i == lhs_len-1 {
                value::new_list(lock_deref!(rhs)[lhs_len-1 ..].to_vec())
            } else {
                lock_deref!(rhs)[i].clone()
            };

        bind_next(context, scopes, names_in_binding, lhs, rhs, None, bind_type)
            .context(BindListItemFailed)?;
    }

    Ok(())
}
