// Copyright 2025-2026 Sean Kelleher. All rights reserved.
// Use of this source code is governed by an MIT
// licence that can be found in the LICENCE file.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use crate::ast::Location;
use crate::value::SourcedValue;

#[derive(Clone, Debug)]
pub struct ScopeStack(Vec<Arc<Mutex<Scope>>>);

pub type Scope = HashMap<String, (SourcedValue, Location, Mutability)>;

#[derive(Debug, PartialEq)]
pub enum Mutability {
    Const,
    Var,
}

#[derive(Debug)]
pub enum Error {
    Const,
    Undefined,
}

impl ScopeStack {
    pub fn new(scopes: Vec<Arc<Mutex<Scope>>>) -> ScopeStack {
        ScopeStack(scopes)
    }

    pub fn new_from_push(&self, scope: Scope) -> ScopeStack {
        let mut scopes = self.0.clone();
        scopes.push(Arc::new(Mutex::new(scope)));

        ScopeStack::new(scopes)
    }

    // `declare` returns `Err` if `name` is already defined in the current
    // scope, and the `Err` will contain the location of the previous
    // definition.
    pub fn declare(
        &mut self,
        name: &str,
        loc: Location,
        v: SourcedValue,
        m: Mutability,
    )
        -> Result<(), Location>
    {
        let mut cur_scope =
            self.0.last()
                .expect("`ScopeStack` stack shouldn't be empty")
                .try_lock()
                .unwrap();

        if let Some((_, loc, _)) = cur_scope.get(name) {
            return Err(*loc);
        }

        cur_scope.insert(name.to_string(), (v, loc, m));

        Ok(())
    }

    pub fn get(&self, name: &String) -> Option<SourcedValue> {
        for scope in self.0.iter().rev() {
            let unlocked_scope = scope.try_lock().unwrap();
            if let Some((v, _, _)) = unlocked_scope.get(name) {
                return Some(v.clone());
            }
        }

        None
    }

    // `assign` replaces `name` in the topmost scope of this `ScopeStack`, or
    // returns an error if `name` wasn't found in this `ScopeStack`, or if
    // `name` refers to a variable defined as a constant.
    pub fn assign(
        &mut self,
        name: &str,
        v: SourcedValue
    ) -> Result<(), Error> {
        for scope in self.0.iter().rev() {
            let mut unlocked_scope = scope.try_lock().unwrap();

            if let Some((slot, _, mutability)) = unlocked_scope.get_mut(name) {
                if *mutability == Mutability::Const {
                    return Err(Error::Const);
                }

                set(slot, v);

                return Ok(());
            }
        }

        Err(Error::Undefined)
    }
}

pub fn set(slot: &mut SourcedValue, v: SourcedValue) {
    *slot = v;
}
