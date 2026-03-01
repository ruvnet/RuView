//! Terms in HoTT (points in type spaces)
//!
//! Terms represent:
//! - Points in spaces (for regular types)
//! - Functions between spaces (for Pi-types)
//! - Pairs of points (for Sigma-types)
//! - Paths between points (for identity types)

use std::fmt;
use super::fresh_id;

/// Terms in HoTT (inhabitants of types)
#[derive(Clone)]
pub enum Term {
    /// Variable reference
    Var(String),

    /// Lambda abstraction: fun x => body
    Lambda {
        var: String,
        body: Box<Term>,
    },

    /// Function application: f(x)
    App {
        func: Box<Term>,
        arg: Box<Term>,
    },

    /// Dependent pair: (a, b) where b may depend on a
    Pair {
        fst: Box<Term>,
        snd: Box<Term>,
    },

    /// First projection: fst(p)
    Fst(Box<Term>),

    /// Second projection: snd(p)
    Snd(Box<Term>),

    /// Reflexivity: refl_a proves a = a
    Refl(Box<Term>),

    /// Transport along a path: transport P p x
    /// Moves x : P(a) to P(b) using p : a = b
    Transport {
        /// Type family P : A -> Type
        family: Box<Term>,
        /// Path p : a = b
        path: Box<Term>,
        /// Term x : P(a)
        term: Box<Term>,
    },

    /// J-eliminator (path induction)
    /// J(A, C, c, a, b, p) where:
    /// - A is the type
    /// - C is the motive: (x y : A) -> (x = y) -> Type
    /// - c is the base case: (x : A) -> C(x, x, refl_x)
    /// - a, b are points, p : a = b
    J {
        motive: Box<Term>,
        base_case: Box<Term>,
        left: Box<Term>,
        right: Box<Term>,
        path: Box<Term>,
    },

    /// Unit value
    Star,

    /// Boolean true
    True,

    /// Boolean false
    False,

    /// Natural number zero
    Zero,

    /// Natural number successor
    Succ(Box<Term>),

    /// Natural number literal
    NatLit(u64),

    /// Natural number recursion: natrec z s n
    /// z : P(0), s : (n : Nat) -> P(n) -> P(S(n))
    NatRec {
        zero_case: Box<Term>,
        succ_case: Box<Term>,
        target: Box<Term>,
    },

    /// Boolean if-then-else
    If {
        cond: Box<Term>,
        then_branch: Box<Term>,
        else_branch: Box<Term>,
    },

    /// Left injection into coproduct
    Inl(Box<Term>),

    /// Right injection into coproduct
    Inr(Box<Term>),

    /// Coproduct elimination (case)
    Case {
        scrutinee: Box<Term>,
        left_case: Box<Term>,
        right_case: Box<Term>,
    },

    /// Empty type elimination (ex falso)
    Abort(Box<Term>),

    /// Path composition: p . q
    PathCompose {
        left: Box<Term>,
        right: Box<Term>,
    },

    /// Path inverse: p^(-1)
    PathInverse(Box<Term>),

    /// Apply function to path: ap f p
    /// If p : a = b and f : A -> B, then ap f p : f(a) = f(b)
    Ap {
        func: Box<Term>,
        path: Box<Term>,
    },

    /// Dependent ap: apd f p
    /// If p : a = b and f : (x : A) -> P(x), then apd f p : transport P p (f a) = f b
    Apd {
        func: Box<Term>,
        path: Box<Term>,
    },

    /// Circle base point
    CircleBase,

    /// Circle loop: loop : base = base
    CircleLoop,

    /// Interval zero endpoint
    IntervalZero,

    /// Interval one endpoint
    IntervalOne,

    /// Truncation introduction
    Truncate(Box<Term>),

    /// Let binding: let x = e1 in e2
    Let {
        var: String,
        value: Box<Term>,
        body: Box<Term>,
    },

    /// Type annotation: (t : T)
    Annot {
        term: Box<Term>,
        ty: Box<super::Type>,
    },

    /// Internal: unique identifier for alpha-equivalence
    #[doc(hidden)]
    InternalId(u64),
}

impl Term {
    /// Create a variable term
    pub fn var(name: &str) -> Self {
        Term::Var(name.to_string())
    }

    /// Create a lambda abstraction
    pub fn lambda(var: &str, body: Term) -> Self {
        Term::Lambda {
            var: var.to_string(),
            body: Box::new(body),
        }
    }

    /// Create a function application
    pub fn app(func: Term, arg: Term) -> Self {
        Term::App {
            func: Box::new(func),
            arg: Box::new(arg),
        }
    }

    /// Create a dependent pair
    pub fn pair(fst: Term, snd: Term) -> Self {
        Term::Pair {
            fst: Box::new(fst),
            snd: Box::new(snd),
        }
    }

    /// Create reflexivity proof
    pub fn refl(term: Term) -> Self {
        Term::Refl(Box::new(term))
    }

    /// Create a natural number from u64
    pub fn nat(n: u64) -> Self {
        Term::NatLit(n)
    }

    /// Substitution: replace variable with term
    pub fn subst(&self, var: &str, replacement: &Term) -> Term {
        match self {
            Term::Var(name) if name == var => replacement.clone(),
            Term::Var(_) => self.clone(),

            Term::Lambda { var: v, body } if v != var => {
                // Avoid variable capture
                let fresh_v = format!("{}_{}", v, fresh_id());
                let body = body.subst(v, &Term::Var(fresh_v.clone()));
                Term::Lambda {
                    var: fresh_v,
                    body: Box::new(body.subst(var, replacement)),
                }
            }
            Term::Lambda { .. } => self.clone(), // var is bound

            Term::App { func, arg } => Term::App {
                func: Box::new(func.subst(var, replacement)),
                arg: Box::new(arg.subst(var, replacement)),
            },

            Term::Pair { fst, snd } => Term::Pair {
                fst: Box::new(fst.subst(var, replacement)),
                snd: Box::new(snd.subst(var, replacement)),
            },

            Term::Fst(p) => Term::Fst(Box::new(p.subst(var, replacement))),
            Term::Snd(p) => Term::Snd(Box::new(p.subst(var, replacement))),

            Term::Refl(t) => Term::Refl(Box::new(t.subst(var, replacement))),

            Term::Transport { family, path, term } => Term::Transport {
                family: Box::new(family.subst(var, replacement)),
                path: Box::new(path.subst(var, replacement)),
                term: Box::new(term.subst(var, replacement)),
            },

            Term::J { motive, base_case, left, right, path } => Term::J {
                motive: Box::new(motive.subst(var, replacement)),
                base_case: Box::new(base_case.subst(var, replacement)),
                left: Box::new(left.subst(var, replacement)),
                right: Box::new(right.subst(var, replacement)),
                path: Box::new(path.subst(var, replacement)),
            },

            Term::Star | Term::True | Term::False | Term::Zero |
            Term::CircleBase | Term::CircleLoop |
            Term::IntervalZero | Term::IntervalOne => self.clone(),

            Term::NatLit(_) | Term::InternalId(_) => self.clone(),

            Term::Succ(n) => Term::Succ(Box::new(n.subst(var, replacement))),

            Term::NatRec { zero_case, succ_case, target } => Term::NatRec {
                zero_case: Box::new(zero_case.subst(var, replacement)),
                succ_case: Box::new(succ_case.subst(var, replacement)),
                target: Box::new(target.subst(var, replacement)),
            },

            Term::If { cond, then_branch, else_branch } => Term::If {
                cond: Box::new(cond.subst(var, replacement)),
                then_branch: Box::new(then_branch.subst(var, replacement)),
                else_branch: Box::new(else_branch.subst(var, replacement)),
            },

            Term::Inl(t) => Term::Inl(Box::new(t.subst(var, replacement))),
            Term::Inr(t) => Term::Inr(Box::new(t.subst(var, replacement))),

            Term::Case { scrutinee, left_case, right_case } => Term::Case {
                scrutinee: Box::new(scrutinee.subst(var, replacement)),
                left_case: Box::new(left_case.subst(var, replacement)),
                right_case: Box::new(right_case.subst(var, replacement)),
            },

            Term::Abort(t) => Term::Abort(Box::new(t.subst(var, replacement))),

            Term::PathCompose { left, right } => Term::PathCompose {
                left: Box::new(left.subst(var, replacement)),
                right: Box::new(right.subst(var, replacement)),
            },

            Term::PathInverse(p) => Term::PathInverse(Box::new(p.subst(var, replacement))),

            Term::Ap { func, path } => Term::Ap {
                func: Box::new(func.subst(var, replacement)),
                path: Box::new(path.subst(var, replacement)),
            },

            Term::Apd { func, path } => Term::Apd {
                func: Box::new(func.subst(var, replacement)),
                path: Box::new(path.subst(var, replacement)),
            },

            Term::Truncate(t) => Term::Truncate(Box::new(t.subst(var, replacement))),

            Term::Let { var: v, value, body } if v != var => Term::Let {
                var: v.clone(),
                value: Box::new(value.subst(var, replacement)),
                body: Box::new(body.subst(var, replacement)),
            },
            Term::Let { var: v, value, body } => Term::Let {
                var: v.clone(),
                value: Box::new(value.subst(var, replacement)),
                body: body.clone(), // var is bound in body
            },

            Term::Annot { term, ty } => Term::Annot {
                term: Box::new(term.subst(var, replacement)),
                ty: ty.clone(),
            },
        }
    }

    /// Get free variables in term
    pub fn free_vars(&self) -> Vec<String> {
        let mut vars = Vec::new();
        self.collect_free_vars(&mut vars, &[]);
        vars
    }

    fn collect_free_vars(&self, vars: &mut Vec<String>, bound: &[String]) {
        match self {
            Term::Var(name) if !bound.contains(name) => {
                if !vars.contains(name) {
                    vars.push(name.clone());
                }
            }
            Term::Var(_) => {}

            Term::Lambda { var, body } => {
                let mut new_bound = bound.to_vec();
                new_bound.push(var.clone());
                body.collect_free_vars(vars, &new_bound);
            }

            Term::App { func, arg } => {
                func.collect_free_vars(vars, bound);
                arg.collect_free_vars(vars, bound);
            }

            Term::Pair { fst, snd } => {
                fst.collect_free_vars(vars, bound);
                snd.collect_free_vars(vars, bound);
            }

            Term::Fst(p) | Term::Snd(p) | Term::Refl(p) |
            Term::Succ(p) | Term::PathInverse(p) | Term::Truncate(p) |
            Term::Inl(p) | Term::Inr(p) | Term::Abort(p) => {
                p.collect_free_vars(vars, bound);
            }

            Term::Transport { family, path, term } => {
                family.collect_free_vars(vars, bound);
                path.collect_free_vars(vars, bound);
                term.collect_free_vars(vars, bound);
            }

            Term::J { motive, base_case, left, right, path } => {
                motive.collect_free_vars(vars, bound);
                base_case.collect_free_vars(vars, bound);
                left.collect_free_vars(vars, bound);
                right.collect_free_vars(vars, bound);
                path.collect_free_vars(vars, bound);
            }

            Term::NatRec { zero_case, succ_case, target } => {
                zero_case.collect_free_vars(vars, bound);
                succ_case.collect_free_vars(vars, bound);
                target.collect_free_vars(vars, bound);
            }

            Term::If { cond, then_branch, else_branch } => {
                cond.collect_free_vars(vars, bound);
                then_branch.collect_free_vars(vars, bound);
                else_branch.collect_free_vars(vars, bound);
            }

            Term::Case { scrutinee, left_case, right_case } => {
                scrutinee.collect_free_vars(vars, bound);
                left_case.collect_free_vars(vars, bound);
                right_case.collect_free_vars(vars, bound);
            }

            Term::PathCompose { left, right } | Term::Ap { func: left, path: right } |
            Term::Apd { func: left, path: right } => {
                left.collect_free_vars(vars, bound);
                right.collect_free_vars(vars, bound);
            }

            Term::Let { var, value, body } => {
                value.collect_free_vars(vars, bound);
                let mut new_bound = bound.to_vec();
                new_bound.push(var.clone());
                body.collect_free_vars(vars, &new_bound);
            }

            Term::Annot { term, .. } => term.collect_free_vars(vars, bound),

            Term::Star | Term::True | Term::False | Term::Zero |
            Term::NatLit(_) | Term::CircleBase | Term::CircleLoop |
            Term::IntervalZero | Term::IntervalOne | Term::InternalId(_) => {}
        }
    }

    /// Check structural equality (alpha-equivalence)
    pub fn structural_eq(&self, other: &Term) -> bool {
        match (self, other) {
            (Term::Var(a), Term::Var(b)) => a == b,
            (Term::Star, Term::Star) => true,
            (Term::True, Term::True) => true,
            (Term::False, Term::False) => true,
            (Term::Zero, Term::Zero) => true,
            (Term::NatLit(a), Term::NatLit(b)) => a == b,
            (Term::CircleBase, Term::CircleBase) => true,
            (Term::CircleLoop, Term::CircleLoop) => true,
            (Term::IntervalZero, Term::IntervalZero) => true,
            (Term::IntervalOne, Term::IntervalOne) => true,

            (Term::Lambda { var: v1, body: b1 }, Term::Lambda { var: v2, body: b2 }) => {
                // Alpha-equivalence: rename variables
                let fresh = format!("alpha_{}", fresh_id());
                let b1_renamed = b1.subst(v1, &Term::Var(fresh.clone()));
                let b2_renamed = b2.subst(v2, &Term::Var(fresh));
                b1_renamed.structural_eq(&b2_renamed)
            }

            (Term::App { func: f1, arg: a1 }, Term::App { func: f2, arg: a2 }) => {
                f1.structural_eq(f2) && a1.structural_eq(a2)
            }

            (Term::Pair { fst: f1, snd: s1 }, Term::Pair { fst: f2, snd: s2 }) => {
                f1.structural_eq(f2) && s1.structural_eq(s2)
            }

            (Term::Fst(p1), Term::Fst(p2)) => p1.structural_eq(p2),
            (Term::Snd(p1), Term::Snd(p2)) => p1.structural_eq(p2),
            (Term::Refl(t1), Term::Refl(t2)) => t1.structural_eq(t2),
            (Term::Succ(n1), Term::Succ(n2)) => n1.structural_eq(n2),
            (Term::Inl(t1), Term::Inl(t2)) => t1.structural_eq(t2),
            (Term::Inr(t1), Term::Inr(t2)) => t1.structural_eq(t2),
            (Term::PathInverse(p1), Term::PathInverse(p2)) => p1.structural_eq(p2),
            (Term::Truncate(t1), Term::Truncate(t2)) => t1.structural_eq(t2),
            (Term::Abort(t1), Term::Abort(t2)) => t1.structural_eq(t2),

            (Term::PathCompose { left: l1, right: r1 }, Term::PathCompose { left: l2, right: r2 }) => {
                l1.structural_eq(l2) && r1.structural_eq(r2)
            }

            (Term::Annot { term: t1, ty: ty1 }, Term::Annot { term: t2, ty: ty2 }) => {
                t1.structural_eq(t2) && ty1.structural_eq(ty2)
            }

            (Term::Transport { family: f1, path: p1, term: t1 },
             Term::Transport { family: f2, path: p2, term: t2 }) => {
                f1.structural_eq(f2) && p1.structural_eq(p2) && t1.structural_eq(t2)
            }

            (Term::J { motive: m1, base_case: b1, left: l1, right: r1, path: p1 },
             Term::J { motive: m2, base_case: b2, left: l2, right: r2, path: p2 }) => {
                m1.structural_eq(m2) && b1.structural_eq(b2) && l1.structural_eq(l2) &&
                r1.structural_eq(r2) && p1.structural_eq(p2)
            }

            (Term::NatRec { zero_case: z1, succ_case: s1, target: t1 },
             Term::NatRec { zero_case: z2, succ_case: s2, target: t2 }) => {
                z1.structural_eq(z2) && s1.structural_eq(s2) && t1.structural_eq(t2)
            }

            (Term::If { cond: c1, then_branch: t1, else_branch: e1 },
             Term::If { cond: c2, then_branch: t2, else_branch: e2 }) => {
                c1.structural_eq(c2) && t1.structural_eq(t2) && e1.structural_eq(e2)
            }

            (Term::Case { scrutinee: s1, left_case: l1, right_case: r1 },
             Term::Case { scrutinee: s2, left_case: l2, right_case: r2 }) => {
                s1.structural_eq(s2) && l1.structural_eq(l2) && r1.structural_eq(r2)
            }

            (Term::Let { var: v1, value: val1, body: b1 },
             Term::Let { var: v2, value: val2, body: b2 }) => {
                v1 == v2 && val1.structural_eq(val2) && b1.structural_eq(b2)
            }

            (Term::Ap { func: f1, path: p1 }, Term::Ap { func: f2, path: p2 }) => {
                f1.structural_eq(f2) && p1.structural_eq(p2)
            }

            (Term::Apd { func: f1, path: p1 }, Term::Apd { func: f2, path: p2 }) => {
                f1.structural_eq(f2) && p1.structural_eq(p2)
            }

            _ => false,
        }
    }
}

impl fmt::Debug for Term {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Term::Var(name) => write!(f, "{}", name),
            Term::Lambda { var, body } => write!(f, "(fun {} => {:?})", var, body),
            Term::App { func, arg } => write!(f, "({:?} {:?})", func, arg),
            Term::Pair { fst, snd } => write!(f, "({:?}, {:?})", fst, snd),
            Term::Fst(p) => write!(f, "fst({:?})", p),
            Term::Snd(p) => write!(f, "snd({:?})", p),
            Term::Refl(t) => write!(f, "refl({:?})", t),
            Term::Transport { family, path, term } => {
                write!(f, "transport({:?}, {:?}, {:?})", family, path, term)
            }
            Term::J { motive, base_case, left, right, path } => {
                write!(f, "J({:?}, {:?}, {:?}, {:?}, {:?})", motive, base_case, left, right, path)
            }
            Term::Star => write!(f, "*"),
            Term::True => write!(f, "true"),
            Term::False => write!(f, "false"),
            Term::Zero => write!(f, "0"),
            Term::Succ(n) => write!(f, "S({:?})", n),
            Term::NatLit(n) => write!(f, "{}", n),
            Term::NatRec { zero_case, succ_case, target } => {
                write!(f, "natrec({:?}, {:?}, {:?})", zero_case, succ_case, target)
            }
            Term::If { cond, then_branch, else_branch } => {
                write!(f, "if {:?} then {:?} else {:?}", cond, then_branch, else_branch)
            }
            Term::Inl(t) => write!(f, "inl({:?})", t),
            Term::Inr(t) => write!(f, "inr({:?})", t),
            Term::Case { scrutinee, left_case, right_case } => {
                write!(f, "case {:?} of inl => {:?} | inr => {:?}", scrutinee, left_case, right_case)
            }
            Term::Abort(t) => write!(f, "abort({:?})", t),
            Term::PathCompose { left, right } => write!(f, "({:?} . {:?})", left, right),
            Term::PathInverse(p) => write!(f, "({:?})^-1", p),
            Term::Ap { func, path } => write!(f, "ap({:?}, {:?})", func, path),
            Term::Apd { func, path } => write!(f, "apd({:?}, {:?})", func, path),
            Term::CircleBase => write!(f, "base"),
            Term::CircleLoop => write!(f, "loop"),
            Term::IntervalZero => write!(f, "i0"),
            Term::IntervalOne => write!(f, "i1"),
            Term::Truncate(t) => write!(f, "|{:?}|", t),
            Term::Let { var, value, body } => {
                write!(f, "let {} = {:?} in {:?}", var, value, body)
            }
            Term::Annot { term, ty } => write!(f, "({:?} : {:?})", term, ty),
            Term::InternalId(id) => write!(f, "#{}", id),
        }
    }
}

impl fmt::Display for Term {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl PartialEq for Term {
    fn eq(&self, other: &Self) -> bool {
        self.structural_eq(other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_substitution() {
        let x = Term::Var("x".to_string());
        let y = Term::Var("y".to_string());

        let result = x.subst("x", &y);
        assert!(matches!(result, Term::Var(name) if name == "y"));
    }

    #[test]
    fn test_free_vars() {
        let term = Term::lambda("x", Term::app(
            Term::Var("x".to_string()),
            Term::Var("y".to_string()),
        ));

        let free = term.free_vars();
        assert_eq!(free, vec!["y"]);
    }

    #[test]
    fn test_alpha_equivalence() {
        let t1 = Term::lambda("x", Term::Var("x".to_string()));
        let t2 = Term::lambda("y", Term::Var("y".to_string()));

        assert!(t1.structural_eq(&t2));
    }
}
