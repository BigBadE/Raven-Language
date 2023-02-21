use pest::iterators::Pairs;
use ast::basic_types::Ident;
use ast::code::{Effect, Expression, MethodCall};
use ast::function::Arguments;
use crate::parser::{EffectParsable, Parsable, Rule};

impl EffectParsable for Box<dyn Effect> {
    fn parse(last: Option<Box<dyn Effect>>, rules: Pairs<Rule>) -> Box<dyn Effect> {
        for element in rules {
            return Box::new(match element.as_rule() {
                Rule::method => MethodCall::parse(last, element.into_inner()),
                _ => panic!("Unimplemented rule!: {}", element)
            });
        }

        panic!("No element in rules?");
    }
}

impl EffectParsable for MethodCall {
    fn parse(last: Option<Box<dyn Effect>>, rules: Pairs<Rule>) -> Self {
        let mut argument = Arguments::default();
        let mut method = String::new();
        for element in rules {
            match element.as_rule() {
                Rule::arguments => argument = Arguments::parse(element.into_inner()),
                Rule::ident => method = element.to_string(),
                _ => panic!("Unimplemented rule!: {}", element)
            }
        }
        return MethodCall::new(last.unwrap(), Ident::new(method), argument)
    }
}

impl Parsable for Arguments {
    fn parse(rules: Pairs<Rule>) -> Self {
        let mut effects = Vec::new();
        for element in rules {
            effects.push(Expression::parse(element.into_inner()));
        }

        return Arguments::new(effects);
    }
}