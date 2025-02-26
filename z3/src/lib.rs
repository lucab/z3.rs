#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(clippy::unreadable_literal)]
#![deny(missing_debug_implementations)]

#[macro_use]
extern crate log;

#[macro_use]
extern crate lazy_static;

extern crate z3_sys;

#[cfg(feature = "arbitrary-size-numeral")]
extern crate num;

use std::ffi::CString;
use std::sync::Mutex;
use z3_sys::*;
pub use z3_sys::{AstKind, GoalPrec, SortKind};

pub mod ast;
mod config;
mod context;
pub mod datatype_builder;
mod func_decl;
mod goal;
mod model;
mod ops;
mod optimize;
mod params;
mod pattern;
mod probe;
mod solver;
mod sort;
mod symbol;
mod tactic;


// Z3 appears to be only mostly-threadsafe, a few initializers
// and such race; so we mutex-guard all access to the library.
lazy_static! {
    static ref Z3_MUTEX: Mutex<()> = Mutex::new(());
}

/// Configuration used to initialize [logical contexts].
///
/// [logical contexts]: struct.Context.html
#[derive(Debug)]
pub struct Config {
    kvs: Vec<(CString, CString)>,
    z3_cfg: Z3_config,
}

/// Manager of all other Z3 objects, global configuration options, etc.
///
/// An application may use multiple Z3 contexts. Objects created in one context
/// cannot be used in another one. However, several objects may be "translated" from
/// one context to another. It is not safe to access Z3 objects from multiple threads.
///
/// # Examples:
///
/// Creating a context with the default configuration:
///
/// ```
/// use z3::{Config, Context};
/// let cfg = Config::new();
/// let ctx = Context::new(&cfg);
/// ```
///
/// [`interrupt()`]: #method.interrupt
#[derive(PartialEq, Eq, Debug)]
pub struct Context {
    z3_ctx: Z3_context,
}

/// Handle that can be used to interrupt a computation from another thread.
#[derive(PartialEq, Eq, Debug)]
pub struct ContextHandle<'ctx> {
    ctx: &'ctx Context,
}

/// Symbols are used to name several term and type constructors.
#[derive(PartialEq, Eq, Clone, Debug)]
pub enum Symbol {
    Int(u32),
    String(String),
}

/// Sorts represent the various 'types' of [`Ast`s](trait.Ast.html).
//
// Note for in-crate users: Never construct a `Sort` directly; only use
// `Sort::new()` which handles Z3 refcounting properly.
pub struct Sort<'ctx> {
    ctx: &'ctx Context,
    z3_sort: Z3_sort,
}

/// a struct to represent when two sorts are of different types
#[derive(Debug)]
pub struct SortDiffers<'ctx> {
  left: Sort<'ctx>,
  right: Sort<'ctx>,
}

/// a struct to represent when an ast is not a function application
#[derive(Debug)]
pub struct IsNotApp {
    kind: AstKind,
}

/// (Incremental) solver, possibly specialized by a particular tactic or logic.
//
// Note for in-crate users: Never construct a `Solver` directly; only use
// `Solver::new()` which handles Z3 refcounting properly.
pub struct Solver<'ctx> {
    ctx: &'ctx Context,
    z3_slv: Z3_solver,
}

/// Model for the constraints inserted into the logical context.
//
// Note for in-crate users: Never construct a `Model` directly; only use
// `Model::new()` which handles Z3 refcounting properly.
pub struct Model<'ctx> {
    ctx: &'ctx Context,
    z3_mdl: Z3_model,
}

/// Context for solving optimization queries.
//
// Note for in-crate users: Never construct an `Optimize` directly; only use
// `Optimize::new()` which handles Z3 refcounting properly.
pub struct Optimize<'ctx> {
    ctx: &'ctx Context,
    z3_opt: Z3_optimize,
}

/// Function declaration. Every constant and function have an associated declaration.
///
/// The declaration assigns a name, a sort (i.e., type), and for function
/// the sort (i.e., type) of each of its arguments. Note that, in Z3,
/// a constant is a function with 0 arguments.
//
// Note for in-crate users: Never construct a `FuncDecl` directly; only use
// `FuncDecl::new()` which handles Z3 refcounting properly.
pub struct FuncDecl<'ctx> {
    ctx: &'ctx Context,
    z3_func_decl: Z3_func_decl,
}

pub use z3_sys::DeclKind;

/// Build a datatype sort.
///
/// Example:
/// ```
/// # use z3::{ast::Int, Config, Context, DatatypeAccessor, DatatypeBuilder, SatResult, Solver, Sort, ast::{Ast, Datatype}};
/// # let cfg = Config::new();
/// # let ctx = Context::new(&cfg);
/// # let solver = Solver::new(&ctx);
/// // Like Rust's Option<int> type
/// let option_int = DatatypeBuilder::new(&ctx, "OptionInt")
/// .variant("None", vec![])
/// .variant(
///     "Some",
///     vec![("value", DatatypeAccessor::Sort(&Sort::int(&ctx)))],
/// )
/// .finish();
///
/// // Assert x.is_none()
/// let x = Datatype::new_const(&ctx, "x", &option_int.sort);
/// solver.assert(&option_int.variants[0].tester.apply(&[&x.into()]).as_bool().unwrap());
///
/// // Assert y == Some(3)
/// let y = Datatype::new_const(&ctx, "y", &option_int.sort);
/// let value = option_int.variants[1].constructor.apply(&[&Int::from_i64(&ctx, 3).into()]);
/// solver.assert(&y._eq(&value.as_datatype().unwrap()));
///
/// assert_eq!(solver.check(), SatResult::Sat);
/// let model = solver.get_model().unwrap();;
///
/// // Get the value out of Some(3)
/// let ast = option_int.variants[1].accessors[0].apply(&[&y.into()]);
/// assert_eq!(3, model.eval(&ast.as_int().unwrap()).unwrap().as_i64().unwrap());
/// ```
#[derive(Debug)]
pub struct DatatypeBuilder<'sort, 'ctx: 'sort> {
    ctx: &'ctx Context,
    name: Symbol,
    constructors: Vec<(String, Vec<(String, DatatypeAccessor<'sort, 'ctx>)>)>,
}

#[derive(Debug)]
pub enum DatatypeAccessor<'sort, 'ctx: 'sort> {
    Sort(&'sort Sort<'ctx>),
    Datatype(Symbol),
}

#[derive(Debug)]
pub struct DatatypeVariant<'ctx> {
    pub constructor: FuncDecl<'ctx>,
    pub tester: FuncDecl<'ctx>,
    pub accessors: Vec<FuncDecl<'ctx>>,
}

#[derive(Debug)]
pub struct DatatypeSort<'ctx> {
    ctx: &'ctx Context,
    pub sort: Sort<'ctx>,
    pub variants: Vec<DatatypeVariant<'ctx>>,
}

pub struct Params<'ctx> {
    ctx: &'ctx Context,
    z3_params: Z3_params,
}

/// Result of a satisfiability query.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SatResult {
    /// The query is unsatisfiable.
    Unsat,
    /// The query was interrupted, timed out or otherwise failed.
    Unknown,
    /// The query is satisfiable.
    Sat,
}

/// A pattern for quantifier instantiation, used to guide quantifier instantiation.
pub struct Pattern<'ctx> {
    ctx: &'ctx Context,
    z3_pattern: Z3_pattern,
}

#[derive(Clone, Debug)]
pub struct ApplyResult<'ctx> {
    ctx: &'ctx Context,
    z3_apply_result: Z3_apply_result,
}

pub struct Tactic<'ctx> {
    ctx: &'ctx Context,
    z3_tactic: Z3_tactic,
}

pub struct Goal<'ctx> {
    ctx: &'ctx Context,
    z3_goal: Z3_goal,
}

pub struct Probe<'ctx> {
    ctx: &'ctx Context,
    z3_probe: Z3_probe,
}
