use crate::cell;
use crate::cell::Cell;
use crate::vm::Error;
use crate::vm::Error::{InvalidDefineSyntax, InvalidSyntax};

macro_rules! car {
    ($cell:expr) => {{
        $cell
            .car()
            .ok_or(Error::ExpectedPairButFound($cell.to_string()))?
    }};
}

macro_rules! cdr {
    ($cell:expr) => {{
        $cell
            .cdr()
            .ok_or(Error::ExpectedPairButFound($cell.to_string()))?
    }};
}

#[derive(Debug, Eq, PartialEq)]
pub struct Transform {
    keyword: Cell,
    ellipsis: Cell,
    syntax_rules: Vec<(Cell, Cell)>,
    literals: Vec<Cell>,
}

impl Transform {
    /// Try New
    ///
    /// Given a (define-syntax ...) expression, build a Transformer
    /// object or return an error.
    ///
    /// # Arguments
    /// `expr` - the full (define-syntax ...) expression
    pub fn try_new(expr: &Cell) -> Result<Transform, Error> {
        let expr = expr.collect_vec();
        let (keyword, mut syntax_rules) = match expr.as_slice() {
            [_, keyword, syntax_rules] => (*keyword, *syntax_rules),
            _ => {
                return Err(InvalidDefineSyntax(
                    "expected keyword and syntax-rules".into(),
                ))
            }
        };

        // keyword must be a symbol
        if !keyword.is_symbol() {
            return Err(InvalidDefineSyntax("keyword must be an identifier".into()));
        }

        // Skip past "syntax-rules"
        if car!(syntax_rules) != &cell!["syntax-rules"] {
            return Err(InvalidDefineSyntax("expected syntax-rules".into()));
        }
        syntax_rules = cdr!(syntax_rules);

        // ellipsis
        let ellipsis = match car!(syntax_rules) {
            Cell::Symbol(_) => {
                let ellipsis = car!(syntax_rules).clone();
                syntax_rules = cdr!(syntax_rules);
                ellipsis
            }
            _ => cell!["..."],
        };

        // literals must be a list of symbols
        let literals = car!(syntax_rules)
            .collect_vec()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();

        if literals.iter().any(|it| !it.is_symbol()) {
            return Err(InvalidDefineSyntax("literals must be identifiers".into()));
        }
        syntax_rules = cdr!(syntax_rules);

        let syntax_rules = syntax_rules.collect_vec();
        let mut syntax_rules_vec = vec![];
        for it in syntax_rules {
            let pattern = car!(it).clone();
            let template = car!(cdr!(it)).clone();
            Self::check_pattern_syntax(&pattern, &ellipsis, &literals)?;
            Self::check_template_syntax(&pattern, &ellipsis, &literals)?;
            syntax_rules_vec.push((pattern, template));
        }

        Ok(Transform {
            keyword: keyword.clone(),
            ellipsis,
            syntax_rules: syntax_rules_vec,
            literals,
        })
    }

    /// Is Literal
    ///
    /// Is cell in the set of literals?
    pub fn is_literal(&self, cell: &Cell) -> bool {
        self.literals.iter().any(|it| it == cell)
    }

    pub fn keyword(&self) -> &Cell {
        &self.keyword
    }

    /// Check Pattern Syntax
    ///
    /// Check numerous rules
    ///
    /// * A variable must not appear more than once in the pattern, unless it is
    ///   a literal or _.
    /// * An ellipsis may only appear once in a list, and must be preceded
    ///   by a pattern variable
    /// * An ellipsis can be the last element in a list, unless it's an improper
    ///   list.
    fn check_pattern_syntax(
        pattern: &Cell,
        ellipsis: &Cell,
        literals: &[Cell],
    ) -> Result<(), Error> {
        if !pattern.is_pair() {
            return Err(InvalidDefineSyntax("pattern must be a ()".into()));
        }
        let mut variables = vec![];
        fn check_pattern_syntax<'a, 'b>(
            pattern: &'a Cell,
            ellipsis: &'a Cell,
            literals: &'a [Cell],
            variables: &'b mut Vec<&'a Cell>,
        ) -> Result<(), Error> {
            if pattern.is_pair() && car!(pattern) == ellipsis {
                return Err(InvalidDefineSyntax("ellipsis out of place".into()));
            }
            let improper = pattern.is_improper_list();
            let mut ellipsis_in_pattern = false;
            let mut cur = pattern.iter().peekable();
            while let Some(pattern) = cur.next() {
                match pattern {
                    Cell::Pair(_, _) => {
                        check_pattern_syntax(pattern, ellipsis, literals, variables)?
                    }
                    Cell::Symbol(sym) => {
                        if literals.iter().any(|it| it == pattern) || sym == "_" {
                            continue;
                        }

                        if pattern == ellipsis {
                            if ellipsis_in_pattern || (improper && cur.peek().is_none()) {
                                return Err(InvalidDefineSyntax("ellipses out of place".into()));
                            }
                            ellipsis_in_pattern = true;
                            continue;
                        }

                        // All other identifiers must be variables
                        if variables.iter().any(|it| *it == pattern) {
                            return Err(InvalidDefineSyntax(format!(
                                "the pattern variable {} was used more than once",
                                pattern
                            )));
                        } else {
                            variables.push(pattern);
                        }
                    }
                    _ => {}
                }
            }

            Ok(())
        }
        check_pattern_syntax(cdr!(pattern), ellipsis, literals, &mut variables)
    }

    /// Check Template Syntax
    fn check_template_syntax(
        _pattern: &Cell,
        _ellipsis: &Cell,
        _literals: &[Cell],
    ) -> Result<(), Error> {
        Ok(())
    }

    /// Find Pattern Variables
    ///
    /// Given the pattern express, build up a set of all pattern
    /// variables:
    ///
    /// * Variables which are not literals
    /// * Variables which are not _
    /// * Variables which are not the ellipsis
    pub fn find_pattern_variables<'a>(&self, pattern: &'a Cell, variables: &mut Vec<&'a Cell>) {
        match pattern {
            Cell::Symbol(_) => {
                if !self.is_literal(pattern) & !(pattern == &cell!["_"])
                    && !(pattern == &self.ellipsis)
                {
                    variables.push(pattern);
                }
            }
            Cell::Pair(_, _) => {
                for it in pattern {
                    self.find_pattern_variables(it, variables);
                }
            }
            _ => {}
        }
    }

    /// Transform
    ///
    /// Transform the input expression given the syntax-rules defined in
    /// this transformer. An error is returned if none of the input expressions
    /// match the patterns specified in the syntax-rules of this transformer.
    ///
    /// # Arguments
    /// `expr` - The expression to transform
    pub fn transform(&self, expr: &Cell) -> Result<Cell, Error> {
        let invalid_syntax = || Err(InvalidSyntax(self.keyword.to_string()));
        if !expr.is_pair() {
            invalid_syntax()?;
        }

        for rule in &self.syntax_rules {
            let mut pattern_variables = vec![];
            self.find_pattern_variables(cdr!(&rule.0), &mut pattern_variables);
            let mut env = PatternEnvironment::new();
            env.variables = pattern_variables;
            if self.pattern_match(cdr!(&rule.0), cdr!(expr), &mut env) {
                return self
                    .expand(&rule.1, &mut env)
                    .ok_or_else(|| InvalidSyntax(self.keyword.to_string()));
            }
        }

        return Err(InvalidSyntax(format!("bad use of {}", self.keyword)));
    }

    /// Pattern Match
    ///
    /// Attempt to match the input expression against one of the syntax-rules pattern,
    /// returning a pattern environment if successful.
    ///
    /// # Arguments
    /// `pattern` - The pattern to attempt to apply
    /// `expr` - The expression to match
    /// `bindings` - The set of matched variable bindings
    fn pattern_match<'a, 'b>(
        &self,
        pattern: &'a Cell,
        expr: &'a Cell,
        env: &'b mut PatternEnvironment<'a>,
    ) -> bool {
        // expr and pattern must either both be lists or improper lists
        if expr.is_pair() && pattern.is_pair() && (expr.is_list() != pattern.is_list()) {
            return false;
        }
        let mut expr_iter = expr.iter().peekable();
        let mut pattern_iter = pattern.iter().peekable();
        if pattern_iter.peek().is_none() {
            return expr_iter.peek().is_none();
        }

        let mut pattern = pattern_iter.next().unwrap();
        loop {
            let in_ellipsis = pattern_iter.peek() == Some(&&self.ellipsis);

            let expr = match expr_iter.next() {
                Some(expr) => expr,
                None => {
                    return in_ellipsis;
                }
            };

            match pattern {
                Cell::Symbol(_) => {
                    env.add_binding(pattern, expr);
                }
                Cell::Pair(_, _) => {
                    if !self.pattern_match(pattern, expr, env) {
                        return false;
                    }
                }
                pattern => {
                    if pattern != expr {
                        return false;
                    }
                }
            }

            if in_ellipsis {
                let pattern_len = pattern_iter.peek().map_or(0, |it| it.len()) - 1;
                let expr_len = expr_iter.peek().map_or(0, |it| it.len());
                if expr_len == pattern_len {
                    pattern_iter.next();
                    pattern = match pattern_iter.next() {
                        Some(pattern) => pattern,
                        None => {
                            return expr_iter.peek().is_none();
                        }
                    }
                }
            } else {
                pattern = match pattern_iter.next() {
                    Some(pattern) => pattern,
                    None => {
                        return expr_iter.peek().is_none();
                    }
                }
            }
        }
    }

    /// Expand
    ///
    /// Given a list of bindings created from a pattern match, and a template, expand
    /// the template with the bindings.
    ///
    /// # Arguments
    /// `template` - The template to use for expansion
    /// `bindings` The matched bindings from the pattern
    fn expand(&self, template: &Cell, env: &mut PatternEnvironment) -> Option<Cell> {
        match template {
            Cell::Symbol(_) => {
                return if env.is_variable(template) {
                    env.get_binding(template).cloned()
                } else {
                    Some(template.clone())
                }
            }
            Cell::Pair(_, _) => {
                let mut v = vec![];
                let mut template_iter = template.iter().peekable();
                let mut template = template_iter.next().unwrap();
                loop {
                    let in_ellipsis = template_iter.peek() == Some(&&self.ellipsis);
                    match self.expand(template, env) {
                        Some(cell) => {
                            v.push(cell);
                            if in_ellipsis {
                                continue;
                            }
                        }
                        None => {
                            if !in_ellipsis {
                                return None;
                            }
                            template_iter.next();
                        }
                    }

                    template = match template_iter.next() {
                        Some(template) => template,
                        None => {
                            break;
                        }
                    };
                }
                Some(Cell::new_list(v))
            }
            cell => Some(cell.clone()),
        }
    }
}

/// Pattern Environment
///
/// Pattern environment is the result of a successful pattern,
/// containing all of the information needed to apply the template
/// portion of the pattern rule.
#[derive(Debug)]
struct PatternEnvironment<'a> {
    /// Bindings are pairs of matched (pattern expr)
    bindings: Vec<(&'a Cell, &'a Cell)>,

    /// Variables are a list of all matching pattern
    /// variable identifiers and can be used to test if
    /// an identifier in the template is a pattern variable
    /// or not.
    variables: Vec<&'a Cell>,
}

impl<'a> PatternEnvironment<'a> {
    fn new() -> PatternEnvironment<'a> {
        PatternEnvironment {
            bindings: vec![],
            variables: vec![],
        }
    }

    fn add_binding(&mut self, pattern: &'a Cell, expr: &'a Cell) {
        self.bindings.push((pattern, expr));
    }

    fn get_binding(&mut self, pattern: &Cell) -> Option<&'a Cell> {
        if let Some(idx) = self.bindings.iter().position(|it| it.0 == pattern) {
            Some(self.bindings.remove(idx).1)
        } else {
            None
        }
    }

    fn is_variable(&self, expr: &Cell) -> bool {
        self.variables.iter().any(|it| *it == expr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;
    use crate::{cell, lex};

    #[test]
    fn error_on_bad_form() {
        assert!(Transform::try_new(&parse!("(define-syntax)")).is_err());
        assert!(Transform::try_new(&parse!("(define-syntax 100)")).is_err());
        assert!(Transform::try_new(&parse!("(define-syntax '())")).is_err());
        assert!(Transform::try_new(&parse!("(define-syntax let not-a-list)")).is_err());
        assert!(
            Transform::try_new(&parse!("(define-syntax let (syntax-rules (1 2 3) ()))")).is_err()
        );
        assert!(Transform::try_new(&parse!(
            r#"        
        (define-syntax begin
              (not-expected-rules ()
                [(begin exp ...)
                 ((lambda () exp ...))]))"#
        ))
        .is_err());
    }

    #[test]
    fn bad_pattern_syntax() {
        // Variable reuse
        assert!(Transform::try_new(&parse!(
            r#"
        (define-syntax bad
              (syntax-rules ()
                [(_ exp exp) ()]))
        "#
        ))
        .is_err());

        assert!(Transform::try_new(&parse!(
            r#"
        (define-syntax bad
              (syntax-rules ()
                [(_ exp . exp) ()]))
        "#
        ))
        .is_err());

        // // nested variable reuse
        assert!(Transform::try_new(&parse!(
            r#"
        (define-syntax bad
              (syntax-rules ()
                [(_ (exp) exp) ()]))
        "#
        ))
        .is_err());
        //
        // // double ellipsis
        assert!(Transform::try_new(&parse!(
            r#"
        (define-syntax bad
              (syntax-rules ()
                [(_ foo ... bar ...) ()]))
        "#
        ))
        .is_err());

        // ellipses out of place
        assert!(Transform::try_new(&parse!(
            r#"
        (define-syntax bad
              (syntax-rules ()
                [(_ (... foo)) ()]))
        "#
        ))
        .is_err());
        assert!(Transform::try_new(&parse!(
            r#"
        (define-syntax bad
              (syntax-rules ()
                [(_ foo . ...) ()]))
        "#
        ))
        .is_err());
    }

    #[test]
    fn alternative_ellipsis_form() {
        let transform = Transform::try_new(&parse!(
            r#"
        (define-syntax begin
              (syntax-rules ___ (bar baz)
                [(begin exp ...)
                 ((lambda () exp ...))]))
        "#
        ))
        .unwrap();
        assert_eq!(transform.ellipsis, cell!["___"]);
        assert_eq!(transform.literals, vec![cell!["bar"], cell!["baz"]]);
    }

    #[test]
    fn literals() {
        let transform = Transform::try_new(&parse!(
            r#"
        (define-syntax begin
              (syntax-rules (bar baz)
                [(begin exp ...)
                 ((lambda () exp ...))]))
        "#
        ))
        .unwrap();
        assert_eq!(transform.keyword, cell!["begin"]);
        assert_eq!(transform.literals, vec![cell!["bar"], cell!["baz"]]);
        assert!(transform.is_literal(&cell!["bar"]));
        assert!(transform.is_literal(&cell!["baz"]));
        assert!(
            Transform::try_new(&parse!("(define-syntax let (syntax-rules (1 2 3) ()))")).is_err()
        );
    }

    #[test]
    fn single_pattern_variable() {
        let transform = Transform::try_new(&parse!(
            r#"        
        (define-syntax bind-zero
              (syntax-rules ()
                [(_ a) (define a 0)]
        ))
        "#
        ))
        .unwrap();
        assert_eq!(
            transform.transform(&parse!("(bind-zero b)")),
            Ok(parse!("(define b 0)"))
        );
    }

    #[test]
    fn nested_pattern_variables() {
        let transform = Transform::try_new(&parse!(
            r#"        
        (define-syntax add-nested
              (syntax-rules ()
                [(_ (x) (y)) (+ x y)]
        ))
        "#
        ))
        .unwrap();
        assert_eq!(
            transform.transform(&parse!("(add-nested (10) (20))")),
            Ok(parse!("(+ 10 20)"))
        );
    }

    #[test]
    fn single_variable_expansion() {
        let transform = Transform::try_new(&parse!(
            r#"
        (define-syntax sum
              (syntax-rules ()
                [(sum *a ...) (+ *a ...)]
        ))
        "#
        ))
        .unwrap();
        assert_eq!(transform.transform(&parse!("(sum)")), Ok(parse!("(+)")));
        assert_eq!(
            transform.transform(&parse!("(sum 10)")),
            Ok(parse!("(+ 10)"))
        );
        assert_eq!(
            transform.transform(&parse!("(sum 10 20)")),
            Ok(parse!("(+ 10 20)"))
        );
    }

    #[test]
    fn begin_macro() {
        let transform = Transform::try_new(&parse!(
            r#"        
        (define-syntax begin
              (syntax-rules ()
                [(begin exp ...)
                 ((lambda () exp ...))]))
        "#
        ))
        .unwrap();
        assert_eq!(transform.keyword, cell!["begin"]);
    }

    #[test]
    fn when_macro() {
        let transform = Transform::try_new(&parse!(
            r#"        
       (define-syntax when
          (syntax-rules ()
            [(when test result1 result2 ...)
             (if test
                 (begin result1 result2 ...))]))                
        "#
        ));
        assert!(transform.is_ok());
    }

    #[test]
    fn and_macro() {
        let transform = Transform::try_new(&parse!(
            r#"        
        (define-syntax and
          (syntax-rules ()
            [(and) #t]
            [(and test) test]
            [(and test1 test2 ...)
             (if test1 (and test2 ...) #f)]))
        "#
        ));
        assert!(transform.is_ok());
    }

    #[test]
    fn or_macro() {
        let transform = Transform::try_new(&parse!(
            r#"        
        (define-syntax or
          (syntax-rules ()
            [(or) #f]
            [(or test) test]
            [(or test1 test2 ...)
             (let ((x test1))
               (if x x (or test2 ...)))]))
        "#
        ));
        assert!(transform.is_ok());
    }

    #[test]
    fn trivial_let_macro() {
        let transform = Transform::try_new(&parse!(
            r#"
        (define-syntax let
            (syntax-rules ()
            [(let ((name val) ...) body1 body2 ...)
                ((lambda (name ...) body1 body2 ...) val ...)]))
        "#
        ));
        assert!(transform.is_ok());
    }
}
