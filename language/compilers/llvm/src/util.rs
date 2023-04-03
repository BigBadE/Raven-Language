use std::collections::HashMap;
use inkwell::values::FunctionValue;
use syntax::function::Function;

pub fn print_formatted(input: String) {
    let mut output = String::new();
    let mut special = false;
    for char in input.chars() {
        if char == '\\' {
            if special {
                output.push('\\');
            }
            special = !special;
        } else if special {
            if char == 'n' {
                output.push('\n');
            } else {
                output.push(char);
            }
            special = false;
        } else {
            output.push(char);
        }
    }
    println!("{}", output);
}