//! Type Checker for HoTT
//!
//! Implements bidirectional type checking with:
//! - Type inference (synthesis)
//! - Type checking
//! - Normalization (beta reduction)
//! - Context management

use std::collections::HashMap;
use super::{Type, Term, TypeError, Level, fresh_id};

/// Typing context
pub type Context = Vec<(String, Type)>;

/// Result of type checking
pub type CheckResult<T> = Result<T, TypeError>;

/// Bidirectional type checker for HoTT
#[derive(Clone)]
pub struct TypeChecker {
    /// Typing context: variable -> type bindings
    context: Context,
    /// Universe level constraints
    level_constraints: HashMap<String, Level>,
    /// Normalization cache
    cache: HashMap<u64, Term>,
}

impl TypeChecker {
    /// Create a new type checker with empty context
    pub fn new() -> Self {
        TypeChecker {
            context: Vec::new(),
            level_constraints: HashMap::new(),
            cache: HashMap::new(),
        }
    }

    /// Create type checker with initial context
    pub fn with_context(&self, ctx: Context) -> Self {
        TypeChecker {
            context: ctx,
            level_constraints: self.level_constraints.clone(),
            cache: HashMap::new(),
        }
    }

    /// Extend context with a new binding
    pub fn extend(&self, var: String, ty: Type) -> Self {
        let mut new_ctx = self.context.clone();
        new_ctx.push((var, ty));
        TypeChecker {
            context: new_ctx,
            level_constraints: self.level_constraints.clone(),
            cache: HashMap::new(),
        }
    }

    /// Look up variable in context
    pub fn lookup(&self, var: &str) -> Option<&Type> {
        self.context.iter().rev()
            .find(|(v, _)| v == var)
            .map(|(_, ty)| ty)
    }

    /// Type checking: verify term has expected type
    pub fn check(&self, term: &Term, expected: &Type) -> CheckResult<()> {
        match (term, expected) {
            // Check lambda against Pi-type
            (Term::Lambda { var, body }, Type::Pi { domain, codomain, .. }) => {
                let extended = self.extend(var.clone(), (**domain).clone());
                let codomain_ty = codomain(&Term::Var(var.clone()));
                extended.check(body, &codomain_ty)
            }

            // Check lambda against arrow type
            (Term::Lambda { var, body }, Type::Arrow(domain, codomain)) => {
                let extended = self.extend(var.clone(), (**domain).clone());
                extended.check(body, codomain)
            }

            // Check pair against Sigma-type
            (Term::Pair { fst, snd }, Type::Sigma { base, fiber, .. }) => {
                self.check(fst, base)?;
                let fiber_ty = fiber(fst);
                self.check(snd, &fiber_ty)
            }

            // Check pair against product type
            (Term::Pair { fst, snd }, Type::Product(left, right)) => {
                self.check(fst, left)?;
                self.check(snd, right)
            }

            // Check reflexivity against identity type
            (Term::Refl(t), Type::Id(ty, left, right)) => {
                self.check(t, ty)?;
                // Verify t equals both left and right
                let t_norm = self.normalize(t);
                let left_norm = self.normalize(left);
                let right_norm = self.normalize(right);

                if !t_norm.structural_eq(&left_norm) || !t_norm.structural_eq(&right_norm) {
                    return Err(TypeError::TypeMismatch {
                        expected: format!("{:?} = {:?}", left, right),
                        found: format!("refl({:?})", t),
                    });
                }
                Ok(())
            }

            // Check star against Unit
            (Term::Star, Type::Unit) => Ok(()),

            // Check true/false against Bool
            (Term::True, Type::Bool) | (Term::False, Type::Bool) => Ok(()),

            // Check zero against Nat
            (Term::Zero, Type::Nat) => Ok(()),

            // Check natural literal against Nat
            (Term::NatLit(_), Type::Nat) => Ok(()),

            // Check successor against Nat
            (Term::Succ(n), Type::Nat) => self.check(n, &Type::Nat),

            // Check injections against coproduct
            (Term::Inl(t), Type::Coprod(left, _)) => self.check(t, left),
            (Term::Inr(t), Type::Coprod(_, right)) => self.check(t, right),

            // Fall back to inference and comparison
            _ => {
                let inferred = self.infer(term)?;
                if self.types_equal(&inferred, expected) {
                    Ok(())
                } else {
                    Err(TypeError::TypeMismatch {
                        expected: format!("{:?}", expected),
                        found: format!("{:?}", inferred),
                    })
                }
            }
        }
    }

    /// Type inference: synthesize the type of a term
    pub fn infer(&self, term: &Term) -> CheckResult<Type> {
        match term {
            // Variable lookup
            Term::Var(name) => {
                self.lookup(name)
                    .cloned()
                    .ok_or_else(|| TypeError::UnboundVariable(name.clone()))
            }

            // Star has type Unit
            Term::Star => Ok(Type::Unit),

            // Booleans
            Term::True | Term::False => Ok(Type::Bool),

            // Naturals
            Term::Zero | Term::NatLit(_) => Ok(Type::Nat),
            Term::Succ(n) => {
                self.check(n, &Type::Nat)?;
                Ok(Type::Nat)
            }

            // Application
            Term::App { func, arg } => {
                let func_ty = self.infer(func)?;
                match func_ty {
                    Type::Pi { domain, codomain, .. } => {
                        self.check(arg, &domain)?;
                        Ok(codomain(arg))
                    }
                    Type::Arrow(domain, codomain) => {
                        self.check(arg, &domain)?;
                        Ok(*codomain)
                    }
                    _ => Err(TypeError::NotAFunction(format!("{:?}", func_ty))),
                }
            }

            // First projection
            Term::Fst(p) => {
                let p_ty = self.infer(p)?;
                match p_ty {
                    Type::Sigma { base, .. } => Ok(*base),
                    Type::Product(left, _) => Ok(*left),
                    _ => Err(TypeError::NotAPair(format!("{:?}", p_ty))),
                }
            }

            // Second projection
            Term::Snd(p) => {
                let p_ty = self.infer(p)?;
                match &p_ty {
                    Type::Sigma { fiber, .. } => {
                        let fst_val = Term::Fst(Box::new((**p).clone()));
                        Ok(fiber(&fst_val))
                    }
                    Type::Product(_, right) => Ok((**right).clone()),
                    _ => Err(TypeError::NotAPair(format!("{:?}", p_ty))),
                }
            }

            // Reflexivity
            Term::Refl(t) => {
                let ty = self.infer(t)?;
                Ok(Type::Id(Box::new(ty), Box::new((**t).clone()), Box::new((**t).clone())))
            }

            // Transport
            Term::Transport { family, path, term: inner } => {
                // Check that path is an identity type
                let path_ty = self.infer(path)?;
                match path_ty {
                    Type::Id(base_ty, source, target) => {
                        // Family should map the base type to types
                        // For simplicity, assume family is well-typed
                        let source_fiber = self.apply_family(family, &source)?;
                        self.check(inner, &source_fiber)?;
                        let target_fiber = self.apply_family(family, &target)?;
                        Ok(target_fiber)
                    }
                    _ => Err(TypeError::InvalidTransport(
                        "Expected identity type".to_string()
                    )),
                }
            }

            // J-eliminator
            Term::J { motive, base_case, left, right, path } => {
                // Verify path type
                let path_ty = self.infer(path)?;
                match path_ty {
                    Type::Id(ty, source, target) => {
                        // Verify left and right match the path
                        if !source.structural_eq(left) || !target.structural_eq(right) {
                            return Err(TypeError::InvalidPathInduction(
                                "Path endpoints don't match".to_string()
                            ));
                        }
                        // The result type is C(left, right, path)
                        // For simplicity, use the base case type
                        self.infer(base_case)
                    }
                    _ => Err(TypeError::InvalidPathInduction(
                        "Expected identity type".to_string()
                    )),
                }
            }

            // If-then-else
            Term::If { cond, then_branch, else_branch } => {
                self.check(cond, &Type::Bool)?;
                let then_ty = self.infer(then_branch)?;
                self.check(else_branch, &then_ty)?;
                Ok(then_ty)
            }

            // Natural number recursion
            Term::NatRec { zero_case, succ_case, target } => {
                self.check(target, &Type::Nat)?;
                let result_ty = self.infer(zero_case)?;
                // Verify succ_case has type Nat -> result_ty -> result_ty
                let expected_succ_ty = Type::arrow(
                    Type::Nat,
                    Type::arrow(result_ty.clone(), result_ty.clone()),
                );
                self.check(succ_case, &expected_succ_ty)?;
                Ok(result_ty)
            }

            // Case analysis on coproduct
            Term::Case { scrutinee, left_case, right_case } => {
                let scrut_ty = self.infer(scrutinee)?;
                match scrut_ty {
                    Type::Coprod(left_ty, right_ty) => {
                        let left_result = self.infer(left_case)?;
                        match left_result {
                            Type::Arrow(_, result) => {
                                // Verify right case has matching type
                                let expected_right = Type::arrow(*right_ty, *result.clone());
                                self.check(right_case, &expected_right)?;
                                Ok(*result)
                            }
                            _ => Err(TypeError::NotAFunction(format!("{:?}", left_result))),
                        }
                    }
                    _ => Err(TypeError::TypeMismatch {
                        expected: "coproduct type".to_string(),
                        found: format!("{:?}", scrut_ty),
                    }),
                }
            }

            // Abort (ex falso)
            Term::Abort(t) => {
                self.check(t, &Type::Empty)?;
                // Can return any type - for inference, return a type variable
                Ok(Type::Var(format!("?{}", fresh_id())))
            }

            // Path composition
            Term::PathCompose { left, right } => {
                let left_ty = self.infer(left)?;
                let right_ty = self.infer(right)?;

                match (&left_ty, &right_ty) {
                    (Type::Id(ty1, a, b), Type::Id(ty2, c, d)) => {
                        if !ty1.structural_eq(ty2) {
                            return Err(TypeError::TypeMismatch {
                                expected: format!("{:?}", ty1),
                                found: format!("{:?}", ty2),
                            });
                        }
                        if !b.structural_eq(c) {
                            return Err(TypeError::PathMismatch {
                                left_target: format!("{:?}", b),
                                right_source: format!("{:?}", c),
                            });
                        }
                        Ok(Type::Id(ty1.clone(), a.clone(), d.clone()))
                    }
                    _ => Err(TypeError::TypeMismatch {
                        expected: "identity types".to_string(),
                        found: format!("{:?} and {:?}", left_ty, right_ty),
                    }),
                }
            }

            // Path inverse
            Term::PathInverse(p) => {
                let p_ty = self.infer(p)?;
                match p_ty {
                    Type::Id(ty, a, b) => Ok(Type::Id(ty, b, a)),  // a and b are already Box<Term>
                    _ => Err(TypeError::TypeMismatch {
                        expected: "identity type".to_string(),
                        found: format!("{:?}", p_ty),
                    }),
                }
            }

            // ap
            Term::Ap { func, path } => {
                let func_ty = self.infer(func)?;
                let path_ty = self.infer(path)?;

                match (&func_ty, &path_ty) {
                    (Type::Arrow(domain, codomain), Type::Id(ty, a, b)) => {
                        if !domain.structural_eq(ty) {
                            return Err(TypeError::TypeMismatch {
                                expected: format!("{:?}", domain),
                                found: format!("{:?}", ty),
                            });
                        }
                        let fa = Term::App {
                            func: Box::new((**func).clone()),
                            arg: a.clone(),
                        };
                        let fb = Term::App {
                            func: Box::new((**func).clone()),
                            arg: b.clone(),
                        };
                        Ok(Type::Id(codomain.clone(), Box::new(fa), Box::new(fb)))
                    }
                    (Type::Pi { domain, codomain, .. }, Type::Id(ty, a, b)) => {
                        if !domain.structural_eq(ty) {
                            return Err(TypeError::TypeMismatch {
                                expected: format!("{:?}", domain),
                                found: format!("{:?}", ty),
                            });
                        }
                        let fa = Term::App {
                            func: Box::new((**func).clone()),
                            arg: a.clone(),
                        };
                        let fb = Term::App {
                            func: Box::new((**func).clone()),
                            arg: b.clone(),
                        };
                        // For Pi-types, compute the codomain at b
                        let result_ty = codomain(&b);
                        Ok(Type::Id(Box::new(result_ty), Box::new(fa), Box::new(fb)))
                    }
                    _ => Err(TypeError::TypeMismatch {
                        expected: "function and identity type".to_string(),
                        found: format!("{:?} and {:?}", func_ty, path_ty),
                    }),
                }
            }

            // Let binding
            Term::Let { var, value, body } => {
                let value_ty = self.infer(value)?;
                let extended = self.extend(var.clone(), value_ty);
                extended.infer(body)
            }

            // Type annotation
            Term::Annot { term: inner, ty } => {
                self.check(inner, ty)?;
                Ok((**ty).clone())
            }

            // Circle
            Term::CircleBase => Ok(Type::Circle),
            Term::CircleLoop => Ok(Type::Id(
                Box::new(Type::Circle),
                Box::new(Term::CircleBase),
                Box::new(Term::CircleBase),
            )),

            // Interval
            Term::IntervalZero | Term::IntervalOne => Ok(Type::Interval),

            // Truncation
            Term::Truncate(t) => {
                let ty = self.infer(t)?;
                Ok(Type::Truncation {
                    inner: Box::new(ty),
                    level: 0, // Default to set-truncation
                })
            }

            // Coproduct injections need type annotation for full inference
            Term::Inl(_) | Term::Inr(_) => {
                Err(TypeError::CannotInfer("injection without type annotation".to_string()))
            }

            // Pair needs type annotation for dependent pairs
            Term::Pair { fst, snd } => {
                let fst_ty = self.infer(fst)?;
                let snd_ty = self.infer(snd)?;
                Ok(Type::Product(Box::new(fst_ty), Box::new(snd_ty)))
            }

            // Lambda needs type annotation
            Term::Lambda { .. } => {
                Err(TypeError::CannotInfer("lambda without type annotation".to_string()))
            }

            // apd
            Term::Apd { func, path } => {
                // Similar to ap but for dependent functions
                let path_ty = self.infer(path)?;
                match path_ty {
                    Type::Id(_, _, _) => {
                        // Result is a dependent path
                        self.infer(func)
                    }
                    _ => Err(TypeError::TypeMismatch {
                        expected: "identity type".to_string(),
                        found: format!("{:?}", path_ty),
                    }),
                }
            }

            Term::InternalId(_) => Err(TypeError::CannotInfer("internal id".to_string())),
        }
    }

    /// Normalize a term (beta reduction)
    pub fn normalize(&self, term: &Term) -> Term {
        match term {
            // Beta reduction for application
            Term::App { func, arg } => {
                let func_norm = self.normalize(func);
                let arg_norm = self.normalize(arg);

                match func_norm {
                    Term::Lambda { var, body } => {
                        let subst = body.subst(&var, &arg_norm);
                        self.normalize(&subst)
                    }
                    _ => Term::App {
                        func: Box::new(func_norm),
                        arg: Box::new(arg_norm),
                    },
                }
            }

            // Projection reduction
            Term::Fst(p) => {
                let p_norm = self.normalize(p);
                match p_norm {
                    Term::Pair { fst, .. } => self.normalize(&fst),
                    _ => Term::Fst(Box::new(p_norm)),
                }
            }

            Term::Snd(p) => {
                let p_norm = self.normalize(p);
                match p_norm {
                    Term::Pair { snd, .. } => self.normalize(&snd),
                    _ => Term::Snd(Box::new(p_norm)),
                }
            }

            // If reduction
            Term::If { cond, then_branch, else_branch } => {
                let cond_norm = self.normalize(cond);
                match cond_norm {
                    Term::True => self.normalize(then_branch),
                    Term::False => self.normalize(else_branch),
                    _ => Term::If {
                        cond: Box::new(cond_norm),
                        then_branch: Box::new(self.normalize(then_branch)),
                        else_branch: Box::new(self.normalize(else_branch)),
                    },
                }
            }

            // Natural recursion reduction
            Term::NatRec { zero_case, succ_case, target } => {
                let target_norm = self.normalize(target);
                match target_norm {
                    Term::Zero | Term::NatLit(0) => self.normalize(zero_case),
                    Term::Succ(n) => {
                        let rec_result = Term::NatRec {
                            zero_case: zero_case.clone(),
                            succ_case: succ_case.clone(),
                            target: n.clone(),
                        };
                        let app1 = Term::App {
                            func: succ_case.clone(),
                            arg: n.clone(),
                        };
                        let app2 = Term::App {
                            func: Box::new(app1),
                            arg: Box::new(rec_result),
                        };
                        self.normalize(&app2)
                    }
                    Term::NatLit(n) if n > 0 => {
                        let pred = Term::NatLit(n - 1);
                        let rec_result = Term::NatRec {
                            zero_case: zero_case.clone(),
                            succ_case: succ_case.clone(),
                            target: Box::new(pred.clone()),
                        };
                        let app1 = Term::App {
                            func: succ_case.clone(),
                            arg: Box::new(pred),
                        };
                        let app2 = Term::App {
                            func: Box::new(app1),
                            arg: Box::new(rec_result),
                        };
                        self.normalize(&app2)
                    }
                    _ => Term::NatRec {
                        zero_case: Box::new(self.normalize(zero_case)),
                        succ_case: Box::new(self.normalize(succ_case)),
                        target: Box::new(target_norm),
                    },
                }
            }

            // Case reduction
            Term::Case { scrutinee, left_case, right_case } => {
                let scrut_norm = self.normalize(scrutinee);
                match scrut_norm {
                    Term::Inl(x) => {
                        let app = Term::App {
                            func: left_case.clone(),
                            arg: x,
                        };
                        self.normalize(&app)
                    }
                    Term::Inr(x) => {
                        let app = Term::App {
                            func: right_case.clone(),
                            arg: x,
                        };
                        self.normalize(&app)
                    }
                    _ => Term::Case {
                        scrutinee: Box::new(scrut_norm),
                        left_case: Box::new(self.normalize(left_case)),
                        right_case: Box::new(self.normalize(right_case)),
                    },
                }
            }

            // Let reduction
            Term::Let { var, value, body } => {
                let value_norm = self.normalize(value);
                let subst = body.subst(var, &value_norm);
                self.normalize(&subst)
            }

            // Path composition with refl
            Term::PathCompose { left, right } => {
                let left_norm = self.normalize(left);
                let right_norm = self.normalize(right);

                match (&left_norm, &right_norm) {
                    (Term::Refl(_), _) => right_norm,
                    (_, Term::Refl(_)) => left_norm,
                    _ => Term::PathCompose {
                        left: Box::new(left_norm),
                        right: Box::new(right_norm),
                    },
                }
            }

            // Path inverse of refl
            Term::PathInverse(p) => {
                let p_norm = self.normalize(p);
                match p_norm {
                    Term::Refl(x) => Term::Refl(x),
                    _ => Term::PathInverse(Box::new(p_norm)),
                }
            }

            // ap on refl
            Term::Ap { func, path } => {
                let func_norm = self.normalize(func);
                let path_norm = self.normalize(path);

                match &path_norm {
                    Term::Refl(x) => {
                        let fx = Term::App {
                            func: Box::new(func_norm),
                            arg: x.clone(),
                        };
                        Term::Refl(Box::new(self.normalize(&fx)))
                    }
                    _ => Term::Ap {
                        func: Box::new(func_norm),
                        path: Box::new(path_norm),
                    },
                }
            }

            // Structural recursion
            Term::Lambda { var, body } => Term::Lambda {
                var: var.clone(),
                body: Box::new(self.normalize(body)),
            },

            Term::Pair { fst, snd } => Term::Pair {
                fst: Box::new(self.normalize(fst)),
                snd: Box::new(self.normalize(snd)),
            },

            Term::Succ(n) => Term::Succ(Box::new(self.normalize(n))),

            Term::Inl(t) => Term::Inl(Box::new(self.normalize(t))),
            Term::Inr(t) => Term::Inr(Box::new(self.normalize(t))),

            Term::Refl(t) => Term::Refl(Box::new(self.normalize(t))),

            Term::Truncate(t) => Term::Truncate(Box::new(self.normalize(t))),

            Term::Annot { term: inner, ty } => Term::Annot {
                term: Box::new(self.normalize(inner)),
                ty: ty.clone(),
            },

            // J-elimination on refl
            Term::J { motive, base_case, left, right, path } => {
                let path_norm = self.normalize(path);
                match &path_norm {
                    Term::Refl(_) => {
                        // J(C, c, a, a, refl_a) = c(a)
                        let app = Term::App {
                            func: base_case.clone(),
                            arg: left.clone(),
                        };
                        self.normalize(&app)
                    }
                    _ => Term::J {
                        motive: Box::new(self.normalize(motive)),
                        base_case: Box::new(self.normalize(base_case)),
                        left: Box::new(self.normalize(left)),
                        right: Box::new(self.normalize(right)),
                        path: Box::new(path_norm),
                    },
                }
            }

            // Transport on refl
            Term::Transport { family, path, term: inner } => {
                let path_norm = self.normalize(path);
                match &path_norm {
                    Term::Refl(_) => self.normalize(inner),
                    _ => Term::Transport {
                        family: Box::new(self.normalize(family)),
                        path: Box::new(path_norm),
                        term: Box::new(self.normalize(inner)),
                    },
                }
            }

            Term::Apd { func, path } => {
                let path_norm = self.normalize(path);
                match &path_norm {
                    Term::Refl(x) => {
                        let fx = Term::App {
                            func: func.clone(),
                            arg: x.clone(),
                        };
                        Term::Refl(Box::new(self.normalize(&fx)))
                    }
                    _ => Term::Apd {
                        func: Box::new(self.normalize(func)),
                        path: Box::new(path_norm),
                    },
                }
            }

            Term::Abort(t) => Term::Abort(Box::new(self.normalize(t))),

            // Values
            Term::Var(_) | Term::Star | Term::True | Term::False |
            Term::Zero | Term::NatLit(_) | Term::CircleBase | Term::CircleLoop |
            Term::IntervalZero | Term::IntervalOne | Term::InternalId(_) => term.clone(),
        }
    }

    /// Check if two types are equal (up to beta-eta equality)
    pub fn types_equal(&self, t1: &Type, t2: &Type) -> bool {
        // First try structural equality
        if t1.structural_eq(t2) {
            return true;
        }

        // For more complex equality, we'd need to normalize type terms
        // For now, use structural equality
        false
    }

    /// Apply a type family (represented as a term) to a term
    fn apply_family(&self, family: &Term, arg: &Term) -> CheckResult<Type> {
        match family {
            Term::Lambda { var, body } => {
                let subst = body.subst(var, arg);
                // Try to interpret the result as a type
                self.term_to_type(&subst)
            }
            _ => {
                // Try applying as a function
                let app = Term::App {
                    func: Box::new(family.clone()),
                    arg: Box::new(arg.clone()),
                };
                self.term_to_type(&self.normalize(&app))
            }
        }
    }

    /// Try to interpret a term as a type
    fn term_to_type(&self, term: &Term) -> CheckResult<Type> {
        match term {
            Term::Var(name) => Ok(Type::Var(name.clone())),
            Term::Annot { ty, .. } => Ok((**ty).clone()),
            _ => {
                // For more complex cases, we'd need a more sophisticated approach
                Ok(Type::Var(format!("{:?}", term)))
            }
        }
    }
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infer_variable() {
        let checker = TypeChecker::new().extend("x".to_string(), Type::Nat);
        let result = checker.infer(&Term::Var("x".to_string()));
        assert!(matches!(result, Ok(Type::Nat)));
    }

    #[test]
    fn test_infer_refl() {
        let checker = TypeChecker::new().extend("x".to_string(), Type::Nat);
        let refl = Term::Refl(Box::new(Term::Var("x".to_string())));
        let result = checker.infer(&refl);

        assert!(matches!(result, Ok(Type::Id(_, _, _))));
    }

    #[test]
    fn test_check_lambda() {
        let checker = TypeChecker::new();
        let identity = Term::Lambda {
            var: "x".to_string(),
            body: Box::new(Term::Var("x".to_string())),
        };
        let id_type = Type::arrow(Type::Nat, Type::Nat);

        assert!(checker.check(&identity, &id_type).is_ok());
    }

    #[test]
    fn test_normalize_beta() {
        let checker = TypeChecker::new();

        // (fun x => x) 42
        let app = Term::App {
            func: Box::new(Term::Lambda {
                var: "x".to_string(),
                body: Box::new(Term::Var("x".to_string())),
            }),
            arg: Box::new(Term::NatLit(42)),
        };

        let result = checker.normalize(&app);
        assert!(matches!(result, Term::NatLit(42)));
    }

    #[test]
    fn test_normalize_proj() {
        let checker = TypeChecker::new();

        // fst (1, 2)
        let pair = Term::Pair {
            fst: Box::new(Term::NatLit(1)),
            snd: Box::new(Term::NatLit(2)),
        };
        let proj = Term::Fst(Box::new(pair));

        let result = checker.normalize(&proj);
        assert!(matches!(result, Term::NatLit(1)));
    }

    #[test]
    fn test_normalize_if() {
        let checker = TypeChecker::new();

        // if true then 1 else 2
        let if_term = Term::If {
            cond: Box::new(Term::True),
            then_branch: Box::new(Term::NatLit(1)),
            else_branch: Box::new(Term::NatLit(2)),
        };

        let result = checker.normalize(&if_term);
        assert!(matches!(result, Term::NatLit(1)));
    }
}
