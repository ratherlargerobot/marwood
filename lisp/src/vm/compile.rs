use crate::cell::Cell;
use crate::vm::node::{Node, Value};
use crate::vm::opcode::OpCode;
use crate::vm::{Error, Vm};
use std::ops::Deref;

macro_rules! car {
    ($cell:expr) => {{
        $cell.car().ok_or(Error::ExpectedPair($cell.to_string()))?
    }};
}

macro_rules! cdr {
    ($cell:expr) => {{
        $cell.cdr().ok_or(Error::ExpectedPair($cell.to_string()))?
    }};
}

impl Vm {
    pub fn compile(&mut self, cell: &Cell) -> Result<Vec<Node>, Error> {
        let mut bc = vec![];
        self.compile_expression(&mut bc, cell)?;
        bc.push(Node::from(OpCode::Halt));
        Ok(bc)
    }

    pub fn compile_expression(&mut self, bc: &mut Vec<Node>, cell: &Cell) -> Result<(), Error> {
        match cell {
            Cell::Pair(car, cdr) => match car.deref() {
                Cell::Symbol(s) if s.eq("quote") => self.compile_quote(bc, car!(cdr))?,
                Cell::Symbol(s) if s.eq("car") => self.compile_car(bc, car!(cdr))?,
                Cell::Symbol(s) if s.eq("cdr") => self.compile_cdr(bc, car!(cdr))?,
                _ => return Err(Error::UnknownProcedure(car.to_string())),
            },
            Cell::Number(_) => self.compile_quote(bc, cell)?,
            _ => return Err(Error::UnknownProcedure(cell.to_string())),
        };
        Ok(())
    }

    pub fn compile_car(&mut self, bc: &mut Vec<Node>, cdr: &Cell) -> Result<(), Error> {
        self.compile_expression(bc, cdr)?;
        bc.push(Node::from(OpCode::Car));
        Ok(())
    }

    pub fn compile_cdr(&mut self, bc: &mut Vec<Node>, cdr: &Cell) -> Result<(), Error> {
        self.compile_expression(bc, cdr)?;
        bc.push(Node::from(OpCode::Cdr));
        Ok(())
    }

    pub fn compile_quote(&mut self, bc: &mut Vec<Node>, cell: &Cell) -> Result<(), Error> {
        bc.push(Node::from(OpCode::Quote));
        bc.push(self.heap.put_cell(cell));
        Ok(())
    }
}

pub fn decompile_text(program: &[Node]) -> String {
    let mut text = String::new();
    for it in decompile(program) {
        text.push_str(&format!("{} {}\n", it.0, it.1.join(",")));
    }
    text
}

pub fn decompile(program: &[Node]) -> Vec<(String, Vec<String>)> {
    let mut cur = program.iter();
    let mut result: Vec<(String, Vec<String>)> = vec![];
    while let Some(node) = cur.next() {
        result.push(match node.val {
            Value::OpCode(ref op) => match op {
                OpCode::Car => ("CAR".into(), vec![]),
                OpCode::Cdr => ("CDR".into(), vec![]),
                OpCode::Halt => ("HALT".into(), vec![]),
                OpCode::Quote => ("QUOTE".into(), vec![cur.next().unwrap().to_string()]),
            },
            _ => ("UNKNOWN".into(), vec![]),
        });
    }
    result
}
