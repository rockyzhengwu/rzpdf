//! The bounded, PDF-only language used by Type4 calculator functions.
//! This is not a general PostScript interpreter.
use crate::error::{PdfError, PdfResult};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Value {
    Integer(i32),
    Real(f64),
    Boolean(bool),
    Procedure(usize, usize),
}

#[derive(Debug, Clone, Copy)]
pub(super) enum Instruction {
    Literal(Value),
    Operator(Operator),
    Procedure(usize, usize),
}

#[derive(Debug, Clone, Copy)]
pub(super) enum Operator {
    Abs,
    Add,
    Atan,
    Ceiling,
    Cos,
    Cvi,
    Cvr,
    Div,
    Exp,
    Floor,
    Idiv,
    Ln,
    Log,
    Mod,
    Mul,
    Neg,
    Round,
    Sin,
    Sqrt,
    Sub,
    Truncate,
    And,
    Bitshift,
    Eq,
    Ge,
    Gt,
    Le,
    Lt,
    Ne,
    Not,
    Or,
    Xor,
    If,
    Ifelse,
    Copy,
    Dup,
    Exch,
    Index,
    Pop,
    Roll,
}

fn error(message: &str) -> PdfError {
    PdfError::FunctionError(format!("Type4 {message}"))
}

fn operator(name: &str) -> Option<Operator> {
    use Operator::*;
    Some(match name {
        "abs" => Abs,
        "add" => Add,
        "atan" => Atan,
        "ceiling" => Ceiling,
        "cos" => Cos,
        "cvi" => Cvi,
        "cvr" => Cvr,
        "div" => Div,
        "exp" => Exp,
        "floor" => Floor,
        "idiv" => Idiv,
        "ln" => Ln,
        "log" => Log,
        "mod" => Mod,
        "mul" => Mul,
        "neg" => Neg,
        "round" => Round,
        "sin" => Sin,
        "sqrt" => Sqrt,
        "sub" => Sub,
        "truncate" => Truncate,
        "and" => And,
        "bitshift" => Bitshift,
        "eq" => Eq,
        "ge" => Ge,
        "gt" => Gt,
        "le" => Le,
        "lt" => Lt,
        "ne" => Ne,
        "not" => Not,
        "or" => Or,
        "xor" => Xor,
        "if" => If,
        "ifelse" => Ifelse,
        "copy" => Copy,
        "dup" => Dup,
        "exch" => Exch,
        "index" => Index,
        "pop" => Pop,
        "roll" => Roll,
        _ => return None,
    })
}

fn whitespace(byte: u8) -> bool {
    matches!(byte, 0 | 9 | 10 | 12 | 13 | 32)
}

fn skip(source: &[u8], offset: &mut usize) {
    loop {
        while source.get(*offset).is_some_and(|byte| whitespace(*byte)) {
            *offset += 1;
        }
        if source.get(*offset) != Some(&b'%') {
            break;
        }
        while source
            .get(*offset)
            .is_some_and(|byte| !matches!(byte, b'\n' | b'\r'))
        {
            *offset += 1;
        }
    }
}

fn push<T>(values: &mut Vec<T>, value: T, nodes: usize, bytes: usize) -> PdfResult<()> {
    let maximum = nodes.min(bytes / std::mem::size_of::<T>());
    if values.len() >= maximum {
        return Err(PdfError::ResourceLimit("Type4 program storage".into()));
    }
    if values.len() == values.capacity() {
        let target = values.capacity().saturating_mul(2).max(1).min(maximum);
        values
            .try_reserve_exact(target - values.len())
            .map_err(|_| PdfError::ResourceLimit("Type4 program allocation".into()))?;
    }
    values.push(value);
    Ok(())
}

pub(super) fn compile(
    source: &[u8],
    nodes: usize,
    bytes: usize,
    depth: usize,
) -> PdfResult<Vec<Instruction>> {
    let mut offset = 0;
    skip(source, &mut offset);
    if source.get(offset) != Some(&b'{') {
        return Err(error("program requires outer braces"));
    }
    offset += 1;
    let mut code = Vec::new();
    // Iterative parser: nesting does not consume the native call stack.
    let mut frames = Vec::new();
    if depth == 0 {
        return Err(PdfError::ResourceLimit("Type4 procedure nesting".into()));
    }
    push(&mut frames, usize::MAX, nodes, bytes)?;
    loop {
        skip(source, &mut offset);
        match source.get(offset).copied() {
            None => return Err(error("unterminated procedure")),
            Some(b'{') => {
                if frames.len() >= depth {
                    return Err(PdfError::ResourceLimit("Type4 procedure nesting".into()));
                }
                let index = code.len();
                push(
                    &mut code,
                    Instruction::Procedure(index + 1, 0),
                    nodes,
                    bytes,
                )?;
                push(&mut frames, index, nodes, bytes)?;
                offset += 1;
            }
            Some(b'}') => {
                offset += 1;
                let index = frames
                    .pop()
                    .ok_or_else(|| error("unexpected closing brace"))?;
                if index == usize::MAX {
                    break;
                }
                code[index] = Instruction::Procedure(index + 1, code.len());
            }
            Some(_) => {
                let begin = offset;
                while source
                    .get(offset)
                    .is_some_and(|byte| !whitespace(*byte) && !matches!(byte, b'{' | b'}' | b'%'))
                {
                    offset += 1;
                }
                let token = std::str::from_utf8(&source[begin..offset])
                    .map_err(|_| error("non-ASCII token"))?;
                let instruction = match token {
                    "true" => Instruction::Literal(Value::Boolean(true)),
                    "false" => Instruction::Literal(Value::Boolean(false)),
                    _ if operator(token).is_some() => {
                        Instruction::Operator(operator(token).unwrap())
                    }
                    _ => {
                        // PDF numeric syntax, not PostScript radix/exponent syntax.
                        if !token
                            .bytes()
                            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'+' | b'-' | b'.'))
                        {
                            return Err(error("unknown operator or invalid number"));
                        }
                        if let Ok(integer) = token.parse::<i32>() {
                            Instruction::Literal(Value::Integer(integer))
                        } else {
                            let number = token
                                .parse::<f64>()
                                .ok()
                                .filter(|number| {
                                    number.is_finite() && number.abs() <= f64::from(f32::MAX)
                                })
                                .ok_or_else(|| error("invalid numeric literal"))?;
                            Instruction::Literal(Value::Real(number))
                        }
                    }
                };
                push(&mut code, instruction, nodes, bytes)?;
            }
        }
    }
    skip(source, &mut offset);
    if offset != source.len() {
        return Err(error("trailing program data"));
    }
    Ok(code)
}

impl Value {
    fn number(self) -> PdfResult<f64> {
        match self {
            Self::Integer(n) => Ok(f64::from(n)),
            Self::Real(n) => Ok(n),
            _ => Err(error("numeric operand required")),
        }
    }
    fn integer(self) -> PdfResult<i32> {
        match self {
            Self::Integer(n) => Ok(n),
            _ => Err(error("integer operand required")),
        }
    }
    fn boolean(self) -> PdfResult<bool> {
        match self {
            Self::Boolean(n) => Ok(n),
            _ => Err(error("boolean operand required")),
        }
    }
    fn procedure(self) -> PdfResult<(usize, usize)> {
        match self {
            Self::Procedure(begin, end) => Ok((begin, end)),
            _ => Err(error("procedure operand required")),
        }
    }
}

fn pop(stack: &mut Vec<Value>) -> PdfResult<Value> {
    stack.pop().ok_or_else(|| error("operand stack underflow"))
}

fn finite(number: f64) -> PdfResult<Value> {
    if number.is_finite() {
        Ok(Value::Real(number))
    } else {
        Err(error("undefined or overflowing arithmetic"))
    }
}

fn convert_integer(number: f64) -> PdfResult<Value> {
    let number = number.trunc();
    if number >= f64::from(i32::MIN) && number <= f64::from(i32::MAX) {
        Ok(Value::Integer(number as i32))
    } else {
        Err(error("integer conversion out of range"))
    }
}

/// Each evaluation owns its stacks. A failure cannot poison a later call.
pub(super) fn evaluate(
    code: &[Instruction],
    inputs: &[f32],
    nodes: usize,
    bytes: usize,
    depth: usize,
) -> PdfResult<Vec<Value>> {
    let mut stack = Vec::new();
    for &input in inputs {
        push(&mut stack, Value::Real(f64::from(input)), nodes, bytes)?;
    }
    if depth == 0 {
        return Err(PdfError::ResourceLimit("Type4 execution nesting".into()));
    }
    let mut frames = Vec::new();
    push(&mut frames, (0usize, code.len()), nodes, bytes)?;
    let mut remaining = nodes;
    while let Some((pc, end)) = frames.last_mut() {
        if *pc == *end {
            frames.pop();
            continue;
        }
        remaining = remaining
            .checked_sub(1)
            .ok_or_else(|| PdfError::ResourceLimit("Type4 execution steps".into()))?;
        let instruction = code
            .get(*pc)
            .copied()
            .ok_or_else(|| error("invalid program counter"))?;
        *pc += 1;
        match instruction {
            Instruction::Literal(value) => push(&mut stack, value, nodes, bytes)?,
            Instruction::Procedure(begin, finish) => {
                *pc = finish;
                push(&mut stack, Value::Procedure(begin, finish), nodes, bytes)?;
            }
            Instruction::Operator(op) => {
                use Operator::*;
                let result = match op {
                    Pop => {
                        pop(&mut stack)?;
                        None
                    }
                    Dup => Some(
                        *stack
                            .last()
                            .ok_or_else(|| error("operand stack underflow"))?,
                    ),
                    Exch => {
                        if stack.len() < 2 {
                            return Err(error("operand stack underflow"));
                        }
                        let length = stack.len();
                        stack.swap(length - 1, length - 2);
                        None
                    }
                    Index => {
                        let index = pop(&mut stack)?.integer()?;
                        if index < 0 || index as usize >= stack.len() {
                            return Err(error("index out of range"));
                        }
                        Some(stack[stack.len() - 1 - index as usize])
                    }
                    Copy => {
                        let count = pop(&mut stack)?.integer()?;
                        if count < 0 || count as usize > stack.len() {
                            return Err(error("copy out of range"));
                        }
                        remaining = remaining.checked_sub(count as usize).ok_or_else(|| {
                            PdfError::ResourceLimit("Type4 execution steps".into())
                        })?;
                        let begin = stack.len() - count as usize;
                        for index in 0..count as usize {
                            let value = stack[begin + index];
                            push(&mut stack, value, nodes, bytes)?;
                        }
                        None
                    }
                    Roll => {
                        let shift = pop(&mut stack)?.integer()?;
                        let count = pop(&mut stack)?.integer()?;
                        if count < 0 || count as usize > stack.len() {
                            return Err(error("roll out of range"));
                        }
                        remaining = remaining.checked_sub(count as usize).ok_or_else(|| {
                            PdfError::ResourceLimit("Type4 execution steps".into())
                        })?;
                        if count != 0 {
                            let begin = stack.len() - count as usize;
                            stack[begin..].rotate_right(
                                i64::from(shift).rem_euclid(i64::from(count)) as usize,
                            );
                        }
                        None
                    }
                    If | Ifelse => {
                        let otherwise = if matches!(op, Ifelse) {
                            Some(pop(&mut stack)?.procedure()?)
                        } else {
                            None
                        };
                        let then = pop(&mut stack)?.procedure()?;
                        let condition = pop(&mut stack)?.boolean()?;
                        let selected = if condition { Some(then) } else { otherwise };
                        if let Some(frame) = selected {
                            if frames.len() >= depth {
                                return Err(PdfError::ResourceLimit(
                                    "Type4 execution nesting".into(),
                                ));
                            }
                            push(&mut frames, frame, nodes, bytes)?;
                        }
                        None
                    }
                    Not => Some(match pop(&mut stack)? {
                        Value::Boolean(value) => Value::Boolean(!value),
                        Value::Integer(value) => Value::Integer(!value),
                        _ => return Err(error("not requires boolean or integer")),
                    }),
                    And | Or | Xor => {
                        let right = pop(&mut stack)?;
                        let left = pop(&mut stack)?;
                        Some(match (left, right) {
                            (Value::Boolean(a), Value::Boolean(b)) => Value::Boolean(match op {
                                And => a & b,
                                Or => a | b,
                                _ => a ^ b,
                            }),
                            (Value::Integer(a), Value::Integer(b)) => Value::Integer(match op {
                                And => a & b,
                                Or => a | b,
                                _ => a ^ b,
                            }),
                            _ => return Err(error("boolean/bitwise operand types differ")),
                        })
                    }
                    Bitshift => {
                        let shift = pop(&mut stack)?.integer()?;
                        let value = pop(&mut stack)?.integer()? as u32;
                        let shifted = if shift >= 32 || shift <= -32 {
                            0
                        } else if shift >= 0 {
                            value << shift as u32
                        } else {
                            value >> (-shift) as u32
                        };
                        Some(Value::Integer(shifted as i32))
                    }
                    Eq | Ne => {
                        let right = pop(&mut stack)?;
                        let left = pop(&mut stack)?;
                        let equal = match (left.number(), right.number()) {
                            (Ok(a), Ok(b)) => a == b,
                            _ => left == right,
                        };
                        Some(Value::Boolean(if matches!(op, Eq) {
                            equal
                        } else {
                            !equal
                        }))
                    }
                    Ge | Gt | Le | Lt => {
                        let right = pop(&mut stack)?.number()?;
                        let left = pop(&mut stack)?.number()?;
                        Some(Value::Boolean(match op {
                            Ge => left >= right,
                            Gt => left > right,
                            Le => left <= right,
                            _ => left < right,
                        }))
                    }
                    Add | Sub | Mul | Div | Idiv | Mod | Exp | Atan => {
                        let right = pop(&mut stack)?;
                        let left = pop(&mut stack)?;
                        let b = right.number()?;
                        let a = left.number()?;
                        let number = match op {
                            Add => a + b,
                            Sub => a - b,
                            Mul => a * b,
                            Div | Idiv => {
                                if b == 0.0 {
                                    return Err(error("division by zero"));
                                }
                                a / b
                            }
                            Mod => {
                                let b = right.integer()?;
                                let a = left.integer()?;
                                if b == 0 {
                                    return Err(error("division by zero"));
                                }
                                (i64::from(a) % i64::from(b)) as f64
                            }
                            Exp => a.powf(b),
                            Atan => {
                                if a == 0.0 && b == 0.0 {
                                    return Err(error("undefined atan"));
                                }
                                a.atan2(b).to_degrees().rem_euclid(360.0)
                            }
                            _ => unreachable!(),
                        };
                        Some(if matches!(op, Idiv | Mod) {
                            convert_integer(number)?
                        } else if matches!(op, Add | Sub | Mul)
                            && matches!((left, right), (Value::Integer(_), Value::Integer(_)))
                            && number >= f64::from(i32::MIN)
                            && number <= f64::from(i32::MAX)
                        {
                            Value::Integer(number as i32)
                        } else {
                            finite(number)?
                        })
                    }
                    Abs | Neg | Ceiling | Floor | Round | Truncate | Sqrt | Sin | Cos | Ln
                    | Log | Cvi | Cvr => {
                        let value = pop(&mut stack)?;
                        let number = value.number()?;
                        let result = match op {
                            Abs => number.abs(),
                            Neg => -number,
                            Ceiling => number.ceil(),
                            Floor => number.floor(),
                            Round => (number + 0.5).floor(),
                            Truncate | Cvi => number.trunc(),
                            Sqrt => number.sqrt(),
                            Sin => number.to_radians().sin(),
                            Cos => number.to_radians().cos(),
                            Ln => number.ln(),
                            Log => number.log10(),
                            Cvr => number,
                            _ => unreachable!(),
                        };
                        Some(if matches!(op, Cvi) {
                            convert_integer(result)?
                        } else if matches!(op, Abs | Neg | Ceiling | Floor | Round | Truncate)
                            && matches!(value, Value::Integer(_))
                            && result >= f64::from(i32::MIN)
                            && result <= f64::from(i32::MAX)
                        {
                            Value::Integer(result as i32)
                        } else {
                            finite(result)?
                        })
                    }
                };
                if let Some(value) = result {
                    push(&mut stack, value, nodes, bytes)?;
                }
            }
        }
    }
    Ok(stack)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(source: &str) -> Vec<Value> {
        let code = compile(source.as_bytes(), 10000, 65536, 16).unwrap();
        evaluate(&code, &[], 10000, 65536, 16).unwrap()
    }

    #[test]
    fn calculator_numeric_operators_preserve_types_and_checked_arithmetic() {
        for (source, expected) in [
            ("{2 3 add}", 5.0),
            ("{2 3 sub}", -1.0),
            ("{2 3 mul}", 6.0),
            ("{7 2 div}", 3.5),
            ("{7 2 idiv}", 3.0),
            ("{-7 2 mod}", -1.0),
            ("{-2 abs}", 2.0),
            ("{2 neg}", -2.0),
            ("{1.2 ceiling}", 2.0),
            ("{-1.2 floor}", -2.0),
            ("{-1.5 round}", -1.0),
            ("{-1.9 truncate}", -1.0),
            ("{9 sqrt}", 3.0),
            ("{90 sin}", 1.0),
            ("{0 cos}", 1.0),
            ("{1 0 atan}", 90.0),
            ("{2 3 exp}", 8.0),
            ("{1 ln}", 0.0),
            ("{100 log}", 2.0),
            ("{1.9 cvi}", 1.0),
            ("{2 cvr}", 2.0),
            ("{2147483647 1 add}", 2147483648.0),
            ("{-2147483648 neg}", 2147483648.0),
            ("{-2147483648 abs}", 2147483648.0),
            ("{-2147483648 -1 mod}", 0.0),
        ] {
            let stack = run(source);
            assert_eq!(stack.len(), 1);
            assert!(
                (stack[0].number().unwrap() - expected).abs() < 0.000001,
                "{source}"
            );
        }
        assert!(matches!(run("{2 3 add}")[0], Value::Integer(5)));
        assert!(matches!(run("{2 cvr}")[0], Value::Real(2.0)));
        assert!(matches!(run("{2147483647 1 add}")[0], Value::Real(_)));
    }

    #[test]
    fn calculator_boolean_comparison_and_bitwise_operators() {
        for source in [
            "{2 2.0 eq}",
            "{2 3 ne}",
            "{2 2 ge}",
            "{3 2 gt}",
            "{2 2 le}",
            "{2 3 lt}",
            "{true false or}",
            "{true true and}",
            "{true false xor}",
            "{false not}",
        ] {
            assert_eq!(run(source), vec![Value::Boolean(true)], "{source}");
        }
        for (source, expected) in [
            ("{6 3 and}", 2),
            ("{6 3 or}", 7),
            ("{6 3 xor}", 5),
            ("{0 not}", -1),
            ("{1 3 bitshift}", 8),
            ("{-1 -1 bitshift}", i32::MAX),
            ("{1 -2147483648 bitshift}", 0),
        ] {
            assert_eq!(run(source), vec![Value::Integer(expected)], "{source}");
        }
    }

    #[test]
    fn calculator_stack_and_conditional_operators() {
        for (source, expected) in [
            ("{1 dup add}", vec![2]),
            ("{1 2 exch}", vec![2, 1]),
            ("{1 2 0 index}", vec![1, 2, 2]),
            ("{1 2 2 copy}", vec![1, 2, 1, 2]),
            ("{1 2 pop}", vec![1]),
            ("{1 2 3 3 1 roll}", vec![3, 1, 2]),
            ("{1 2 3 3 -1 roll}", vec![2, 3, 1]),
            ("{0 7 roll}", vec![]),
            ("{true {1} if}", vec![1]),
            ("{false {1} if}", vec![]),
            ("{true {1} {2} ifelse}", vec![1]),
            ("{false {1} {2} ifelse}", vec![2]),
            ("{true {false {1} {2} ifelse} if}", vec![2]),
        ] {
            assert_eq!(
                run(source),
                expected.into_iter().map(Value::Integer).collect::<Vec<_>>(),
                "{source}"
            );
        }
    }

    #[test]
    fn calculator_execution_errors_and_limits_do_not_poison_retry() {
        for source in [
            "{add}",
            "{true 1 add}",
            "{1 0 div}",
            "{1 0 idiv}",
            "{1 0 mod}",
            "{-1 sqrt}",
            "{0 ln}",
            "{-1 log}",
            "{0 0 atan}",
            "{2147483648 cvi}",
            "{-2147483648 -1 idiv}",
            "{true 1 and}",
            "{1 {2} if}",
            "{2 copy}",
            "{-1 index}",
            "{1 2 3 roll}",
        ] {
            let code = compile(source.as_bytes(), 100, 4096, 8).unwrap();
            assert!(
                matches!(
                    evaluate(&code, &[], 100, 4096, 8),
                    Err(PdfError::FunctionError(_))
                ),
                "{source}"
            );
        }
        let code = compile(b"{1 dup dup}", 100, 4096, 8).unwrap();
        assert!(matches!(
            evaluate(&code, &[], 2, 4096, 8),
            Err(PdfError::ResourceLimit(_))
        ));
        assert!(evaluate(&code, &[], 3, 4096, 8).is_ok());
        assert!(matches!(
            evaluate(&code, &[], 100, std::mem::size_of::<Value>() * 2, 8),
            Err(PdfError::ResourceLimit(_))
        ));
        let code = compile(b"{true {true {1} if} if}", 100, 4096, 8).unwrap();
        assert!(matches!(
            evaluate(&code, &[], 100, 4096, 2),
            Err(PdfError::ResourceLimit(_))
        ));
        assert!(evaluate(&code, &[], 100, 4096, 3).is_ok());
    }

    #[test]
    fn calculator_compiles_comments_numbers_and_nested_procedures() {
        let code = compile(
            b"% before\n{ .5 -2 true {dup 1 add} if % after\n }",
            100,
            4096,
            8,
        )
        .unwrap();
        assert_eq!(code.len(), 8);
        assert!(matches!(code[3], Instruction::Procedure(4, 7)));
        assert!(matches!(code[0], Instruction::Literal(Value::Real(0.5))));
    }

    #[test]
    fn calculator_rejects_non_pdf_tokens_and_structure() {
        for source in [
            "", "1", "{", "{{}", "{} }", "{} 1", "{1e2}", "{16#FF}", "{run}", "{(text)}", "{NaN}",
            "{--1}",
        ] {
            assert!(
                matches!(
                    compile(source.as_bytes(), 100, 4096, 8),
                    Err(PdfError::FunctionError(_))
                ),
                "{source}"
            );
        }
    }

    #[test]
    fn calculator_program_and_nesting_limits_are_exact() {
        assert!(compile(b"{1 2 add}", 3, 4096, 1).is_ok());
        assert!(matches!(
            compile(b"{1 2 add}", 2, 4096, 1),
            Err(PdfError::ResourceLimit(_))
        ));
        assert!(compile(b"{{}}", 100, 4096, 2).is_ok());
        assert!(matches!(
            compile(b"{{}}", 100, 4096, 1),
            Err(PdfError::ResourceLimit(_))
        ));
        assert!(compile(b"{}", 3, 8, 1).is_ok());
    }

    #[test]
    fn calculator_literal_range_and_hundred_operand_stack() {
        assert!(matches!(
            compile(b"{400000000000000000000000000000000000000}", 100, 4096, 8),
            Err(PdfError::FunctionError(_))
        ));
        let source = format!("{{ {} }}", "0 ".repeat(100));
        let code = compile(source.as_bytes(), 100, 65536, 8).unwrap();
        let bytes = std::mem::size_of::<Value>() * 100;
        assert_eq!(evaluate(&code, &[], 100, bytes, 8).unwrap().len(), 100);
        assert!(matches!(
            evaluate(&code, &[], 100, bytes - std::mem::size_of::<Value>(), 8),
            Err(PdfError::ResourceLimit(_))
        ));
    }
}
