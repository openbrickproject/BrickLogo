use crate::token::Token;

#[derive(Debug, Clone)]
pub enum AstNode {
    Number(f64),
    Word(String),
    Variable(String),
    List(Vec<AstNode>),
    Infix { operator: String, left: Box<AstNode>, right: Box<AstNode> },
    Paren { name: String, args: Vec<AstNode> },
    Call { name: String, args: Vec<AstNode>, token: Token },
    ProcDef { name: String, params: Vec<String>, body: Vec<AstNode> },
    Repeat { count: Box<AstNode>, body: Vec<AstNode> },
    Forever { body: Vec<AstNode> },
    If { condition: Box<AstNode>, body: Vec<AstNode> },
    IfElse { condition: Box<AstNode>, then_body: Vec<AstNode>, else_body: Vec<AstNode> },
    WaitUntil { condition: Vec<AstNode> },
    Carefully { body: Vec<AstNode>, handler: Vec<AstNode> },
    Output(Box<AstNode>),
    Stop,
}

#[derive(Debug, Clone)]
pub struct UserProcedure {
    pub name: String,
    pub params: Vec<String>,
    pub body: Vec<AstNode>,
}
