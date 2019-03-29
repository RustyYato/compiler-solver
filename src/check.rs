use super::{InfVar, Predicate, Quant, Rule, Solver};

use std::collections::HashSet;

trait COpt<T>: Sized {
    fn call_with<F: FnOnce(T)>(self, _: F) {}
}

struct CSome<T>(T);
struct CNone;

impl<T> COpt<T> for CSome<T> {
    fn call_with<F: FnOnce(T)>(self, f: F) {
        f(self.0)
    }
}
impl<T> COpt<T> for CNone {}

pub struct Token<'a>(&'a ());

impl<P: Predicate> Solver<P> {
    pub fn is_consistent(&self) -> Option<Token<'_>> {
        if self.is_consistent_raw(CNone) {
            Some(Token(&()))
        } else {
            None
        }
    }

    pub fn is_consistent_with_rule(&self, rule: Rule<P>, _: &Token<'_>) -> bool {
        self.is_consistent_raw(CSome(rule))
    }

    fn is_consistent_raw<O: COpt<Rule<P>>>(&self, new_rule: O) -> bool {
        struct Existential;
        struct NotExistential;

        trait HandleImplicationUndetermined {
            fn handle() -> Option<()>;
        }

        impl HandleImplicationUndetermined for Existential {
            #[inline(always)]
            fn handle() -> Option<()> {
                None
            }
        }

        impl HandleImplicationUndetermined for NotExistential {
            #[inline(always)]
            fn handle() -> Option<()> {
                Some(())
            }
        }

        fn is_consistent_inner<H: HandleImplicationUndetermined, P: Predicate>(
            rules: &mut Vec<Rule<P>>,
            axioms: &mut HashSet<P>,
            known_variables: &HashSet<P::Item>,
        ) -> Option<()> {
            struct OnDrop;
            impl Drop for OnDrop {
                fn drop(&mut self) {
                    println!("}}--", );
                }
            }

            while let Some(rule) = rules.pop() {
                println!("--{{", );
                let _on_drop = OnDrop;
                dbg!(&axioms);

                match dbg!(rule) {
                    Rule::Axiom(x) => {
                        dbg!("axiom");
                        if axioms.contains(&x.not()) {
                            return None;
                        } else {
                            axioms.insert(x);
                        }
                    }
                    Rule::And(box [a, b]) => {
                        dbg!("and");
                        rules.push(a);
                        rules.push(b);
                    }
                    Rule::Implication(box [a, b]) => {
                        dbg!("implication");
                        if let Some(true) = a.eval(&axioms) {
                            fn add_to_axioms<P: Predicate>(axioms: &mut HashSet<P>, add_axiom: Rule<P>) {
                                match dbg!(add_axiom) {
                                    Rule::Axiom(x) => {
                                        axioms.insert(x);
                                    },
                                    Rule::And(box [a, b]) => {
                                        add_to_axioms(axioms, a);
                                        add_to_axioms(axioms, b);
                                    },
                                    Rule::Implication(box [a, b]) => {
                                        if let Some(true) = a.eval(&axioms) {
                                            add_to_axioms(axioms, b)
                                        }
                                    },
                                    Rule::Quantifier(..) => unimplemented!()
                                }
                            }

                            match dbg!(b.eval(&axioms)) {
                                // if guaranteed false, then return false
                                Some(false) => return None,

                                // if guaranteed true , then return true
                                Some(true) => add_to_axioms(axioms, b),

                                // if undetermined    , then return false if existential
                                // if undetermined    , then return true if !existential
                                None => {
                                    H::handle()?;

                                    add_to_axioms(axioms, b);
                                },
                            }
                        }
                    }
                    Rule::Quantifier(Quant::ForAll, inf_var, rule) => {
                        dbg!("forall");
                        for var in known_variables {
                            let rule = rule.apply(inf_var, var);

                            // This needs to be tested.
                            // The trade off of this line is
                            // will the number of variables or
                            // the number of forall quantifiers
                            // dominate performance?
                            dbg!(is_consistent_inner::<H, _>(&mut vec![rule], axioms, known_variables))?;
                        }
                    }
                    Rule::Quantifier(Quant::Exists, inf_var, rule) => {
                        dbg!("exists");
                        let mut iter = known_variables.iter();

                        loop {
                            let var = iter.next()?;

                            let rule = rule.apply(inf_var, var);

                            let is_inner_consistent = is_consistent_inner::<Existential, _>(
                                &mut vec![rule],
                                axioms,
                                known_variables,
                            );

                            if is_inner_consistent.is_some() {
                                break;
                            }
                        }
                    }
                }
            }

            Some(())
        }

        let mut rules = self.rules.iter().cloned().collect::<Vec<_>>();

        new_rule.call_with(|x| rules.push(x));

        rules.sort_unstable_by_key(|x| match x {
            Rule::Axiom(..) => 4,
            Rule::And(_) => 3,
            Rule::Implication(..) => 2,
            Rule::Quantifier(Quant::ForAll, ..) => 1,
            Rule::Quantifier(Quant::Exists, ..) => 0,
        });

        let mut forall_pos = None;
        let mut exists_pos = None;

        for (i, a) in rules.windows(2).enumerate() {
            let i = i + 1;

            let (a, b) = match a {
                [a, b] => (a, b),
                _ => unreachable!()
            };

            match dbg!((a, b)) {
                (Rule::Quantifier(q, ..), Rule::Quantifier(r, ..)) if q == r => (),
                (Rule::Quantifier(Quant::Exists, ..), _) => {
                    exists_pos = Some(i);
                },
                (Rule::Quantifier(Quant::ForAll, ..), _) => {
                    forall_pos = Some(i);
                    break
                },
                _ => ()
            }
        }

        fn sort_rules_and_find_cycle<P: Predicate>(rules: &mut [Rule<P>]) -> bool {
            let mut found_cycle = false;
            
            rules.sort_by(|x, y| {
                use std::cmp::Ordering;

                if found_cycle {
                    return Ordering::Equal;
                }

                dbg!(match dbg!((x, y)) {
                    (
                        Rule::Quantifier(_, _, box Rule::Implication(box [a, b])),
                        Rule::Quantifier(_, _, box Rule::Implication(box [c, d]))
                    ) => {
                        match dbg!((d.contains(a), b.contains(c))) {
                            (true, true) => {
                                found_cycle = true;
                                Ordering::Equal
                            },
                            (true, false) => Ordering::Less,
                            (false, true) => Ordering::Greater,
                            (false, false) => Ordering::Equal
                        }
                    },
                    (Rule::Quantifier(..), Rule::Quantifier(..)) => {
                        Ordering::Equal
                    },
                    _ => unreachable!()
                })
            });

            found_cycle
        }

        match dbg!((exists_pos, forall_pos)) {
            (Some(exists_pos), forall_pos) => {
                sort_rules_and_find_cycle(&mut rules[..exists_pos]);

                if let Some(forall_pos) = forall_pos {
                    if sort_rules_and_find_cycle(&mut rules[exists_pos..forall_pos]) {
                        return false;
                    }
                }
            },
            (None, Some(forall_pos)) => {
                if sort_rules_and_find_cycle(&mut rules[..forall_pos]) {
                    return false;
                }
            }
            (None, None) => ()
        }

        dbg!("-----------------------------------------------------------------------------------");

        dbg!(&rules);

        match rules.last() {
            Some(Rule::Implication(..)) | Some(Rule::Quantifier(..)) => {
                for i in rules.iter() {
                    if let Rule::Quantifier(Quant::Exists, ..) = i {
                        return false;
                    }
                }
            }
            _ => (),
        }

        let known_variables = rules.iter().flat_map(Rule::items).collect();

        is_consistent_inner::<NotExistential, _>(
            &mut rules,
            &mut Default::default(),
            &known_variables,
        )
        .is_some()
    }
}

impl<P: Predicate> Rule<P> {
    fn apply(&self, v: InfVar, i: &P::Item) -> Self {
        match self {
            &Rule::Axiom(ref x) => Rule::Axiom(x.apply(v, i)),
            Rule::Implication(box [a, b]) => {
                Rule::Implication(Box::new([a.apply(v, i), b.apply(v, i)]))
            }
            Rule::And(box [a, b]) => Rule::And(Box::new([a.apply(v, i), b.apply(v, i)])),
            &Rule::Quantifier(q, qv, ref rule) => {
                Rule::Quantifier(q, qv, Box::new(rule.apply(v, i)))
            }
        }
    }

    fn eval(&self, axioms: &HashSet<P>) -> Option<bool> {
        match self {
            &Rule::Axiom(ref x) => {
                if axioms.contains(x) {
                    Some(true)
                } else if axioms.contains(&x.not()) {
                    Some(false)
                } else {
                    None
                }
            }
            Rule::Implication(box [a, b]) => Some(!a.eval(axioms)? || b.eval(axioms)?),
            Rule::And(box [a, b]) => Some(a.eval(axioms)? && b.eval(axioms)?),
            Rule::Quantifier(..) => unimplemented!(),
        }
    }

    fn items<'a>(&'a self) -> Box<dyn 'a + Iterator<Item = P::Item>> {
        match self {
            Rule::Axiom(x) => Box::new(x.items()),

            Rule::Implication(box [a, b]) | Rule::And(box [a, b]) => {
                Box::new(a.items().chain(b.items()))
            }

            Rule::Quantifier(_, _, box rule) => rule.items(),
        }
    }

    fn contains(&self, rule: &Self) -> bool {
        if self == rule {
            true
        } else {
            match dbg!(self) {
                Rule::Axiom(s) => {
                    match dbg!(rule) {
                        Rule::Axiom(r) => s.matches(r),
                        | Rule::Implication(box [a, b])
                        | Rule::And(box [a, b]) => self.contains(a) || self.contains(b),
                        Rule::Quantifier(_, _, rule) => self.contains(rule)
                    }
                },
                Rule::And(box [a, b]) => a.contains(rule) || b.contains(rule),
                Rule::Implication(box [a, b]) => {
                    match dbg!(rule) {
                        | Rule::And(..)
                        | Rule::Axiom(..) => a.contains(rule) || b.contains(rule),
                        Rule::Implication(box [c, d]) => a.contains(c) && b.contains(d),
                        Rule::Quantifier(_, _, rule) => a.contains(rule) || b.contains(rule),
                    }
                },
                Rule::Quantifier(..) => {
                    unimplemented!()
                }
            }
        }
    }
}
