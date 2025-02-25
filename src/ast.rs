// Copyright 2025 Sean Kelleher. All rights reserved.
// Use of this source code is governed by an MIT
// licence that can be found in the LICENCE file.

#[derive(Clone, Debug)]
pub enum Prog {
    Body{stmts: Block},
}

pub type Block = Vec<Stmt>;

#[derive(Clone, Debug)]
pub enum Stmt {
    Block{block: Block},

    Expr{expr: Expr},

    Declare{lhs: Expr, rhs: Expr},
    Assign{lhs: Expr, rhs: Expr},
    OpAssign{
        lhs: Expr,
        op: BinaryOp,
        op_loc: Location,
        rhs: Expr,
    },

    If{branches: Vec<Branch>, else_stmts: Option<Block>},

    While{cond: Expr, stmts: Block},
    For{lhs: Expr, iter: Expr, stmts: Block},
    Break{loc: Location},
    Continue{loc: Location},

    Func{
        name: (String, Location),
        args: Vec<Expr>,
        collect_args: bool,
        stmts: Block,
    },
    Return{loc: Location, expr: Expr},
}

#[derive(Clone,Debug)]
pub struct Branch {
    pub cond: Expr,
    pub stmts: Block,
}

pub type Location = (usize, usize);

pub type Expr = (RawExpr, Location);

#[derive(Clone, Debug)]
pub enum RawExpr {
    Null,

    Bool{b: bool},
    Int{n: i64},
    // `interpolation_slots` is `None` iff the string isn't interpolated,
    // otherwise it contains start/end indices of substrings to be evaluated
    // during interpolation.
    Str{s: String, interpolation_slots: Option<Vec<(usize, usize)>>},

    Var{name: String},

    UnaryOp{
        op: UnaryOp,
        op_loc: Location,
        expr: Box<Expr>,
    },

    BinaryOp{
        op: BinaryOp,
        op_loc: Location,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },

    List{items: Vec<ListItem>, collect: bool},
    Index{expr: Box<Expr>, location: Box<Expr>},
    RangeIndex{
        expr: Box<Expr>,
        start: Option<Box<Expr>>,
        end: Option<Box<Expr>>,
    },
    Range{start: Box<Expr>, end: Box<Expr>},

    Object{props: Vec<PropItem>},
    Prop{expr: Box<Expr>, name: String, type_prop: bool},

    Func{args: Vec<Expr>, collect_args: bool, stmts: Block},
    Call{func: Box<Expr>, args: Vec<ListItem>},

    CatchAsBool{expr: Box<Expr>},
}

#[derive(Clone, Debug)]
pub enum UnaryOp {
    Not,
}

#[derive(Clone, Debug)]
pub enum BinaryOp {
    Sum,
    Sub,
    Mul,
    Div,
    Mod,

    And,
    Or,

    Eq,
    Ne,
    Gt,
    Gte,
    Lt,
    Lte,

    RefEq,
    RefNe,
}

#[derive(Clone,Debug)]
pub struct ListItem {
    pub expr: Expr,
    pub is_spread: bool,
}

#[derive(Clone, Debug)]
pub enum PropItem {
    Pair{name: Expr, value: Expr},
    Single{expr: Expr, is_spread: bool, collect: bool},
}
