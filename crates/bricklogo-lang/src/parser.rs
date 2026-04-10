use crate::ast::AstNode;
use crate::error::{LogoError, LogoResult};
use crate::token::{Token, TokenType};
use std::collections::HashMap;

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    /// Known procedure arities: name -> arg count
    arities: HashMap<String, usize>,
}

impl Parser {
    pub fn new(arities: HashMap<String, usize>) -> Self {
        Parser {
            tokens: Vec::new(),
            pos: 0,
            arities,
        }
    }

    pub fn set_arity(&mut self, name: &str, arity: usize) {
        self.arities.insert(name.to_lowercase(), arity);
    }

    pub fn parse(&mut self, tokens: Vec<Token>) -> LogoResult<Vec<AstNode>> {
        self.tokens = tokens;
        self.pos = 0;
        let mut nodes = Vec::new();
        self.skip_newlines();
        while !self.is_at_end() {
            nodes.push(self.parse_expression()?);
            self.skip_newlines();
        }
        Ok(nodes)
    }

    fn parse_expression(&mut self) -> LogoResult<AstNode> {
        let mut left = self.parse_atom()?;
        while self.peek().token_type == TokenType::Infix {
            let op = self.advance();
            let right = self.parse_atom()?;
            left = AstNode::Infix {
                operator: op.value,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_atom(&mut self) -> LogoResult<AstNode> {
        let token = self.peek().clone();
        match token.token_type {
            TokenType::Number => {
                self.advance();
                Ok(AstNode::Number(token.value.parse::<f64>().unwrap()))
            }
            TokenType::QuotedWord => {
                self.advance();
                Ok(AstNode::Word(token.value))
            }
            TokenType::Variable => {
                self.advance();
                Ok(AstNode::Variable(token.value))
            }
            TokenType::OpenBracket => self.parse_list(),
            TokenType::OpenParen => self.parse_paren_expr(),
            TokenType::Word => self.parse_word_expr(),
            _ => Err(LogoError::Syntax {
                message: format!("Unexpected token '{}'", token.value),
                line: token.line,
                col: token.col,
            }),
        }
    }

    fn parse_word_expr(&mut self) -> LogoResult<AstNode> {
        let token = self.advance();
        let name = &token.value;

        match name.as_str() {
            "to" => self.parse_procedure_def(&token),
            "repeat" => self.parse_repeat(),
            "forever" => self.parse_forever(),
            "launch" => self.parse_launch(),
            "if" => self.parse_if(),
            "ifelse" => self.parse_ifelse(),
            "waituntil" => self.parse_waituntil(),
            "carefully" => self.parse_carefully(),
            "output" | "op" => {
                let value = self.parse_expression()?;
                Ok(AstNode::Output(Box::new(value)))
            }
            "stop" => Ok(AstNode::Stop),
            "true" => Ok(AstNode::Word("true".to_string())),
            "false" => Ok(AstNode::Word("false".to_string())),
            _ => {
                let arity = self.arities.get(&name.to_lowercase()).copied().unwrap_or(0);
                let mut args = Vec::new();
                for i in 0..arity {
                    if self.is_at_end() || self.peek().token_type == TokenType::Newline {
                        return Err(LogoError::Syntax {
                            message: format!("{} needs {} input(s), but got {}", name, arity, i),
                            line: token.line,
                            col: token.col,
                        });
                    }
                    args.push(self.parse_expression()?);
                }
                Ok(AstNode::Call {
                    name: name.to_string(),
                    args,
                    token,
                })
            }
        }
    }

    fn parse_procedure_def(&mut self, token: &Token) -> LogoResult<AstNode> {
        if self.peek().token_type != TokenType::Word {
            return Err(LogoError::Syntax {
                message: "Expected procedure name after 'to'".to_string(),
                line: token.line,
                col: token.col,
            });
        }
        let name_token = self.advance();
        let name = name_token.value;
        let mut params = Vec::new();

        while self.peek().token_type == TokenType::Variable {
            params.push(self.advance().value);
        }

        // Register arity eagerly for recursive calls
        self.arities.insert(name.clone(), params.len());

        self.skip_newlines();

        let mut body = Vec::new();
        while !self.is_at_end() {
            if self.peek().token_type == TokenType::Word && self.peek().value == "end" {
                self.advance();
                return Ok(AstNode::ProcDef { name, params, body });
            }
            body.push(self.parse_expression()?);
            self.skip_newlines();
        }

        Err(LogoError::Syntax {
            message: format!("Missing 'end' for procedure '{}'", name),
            line: token.line,
            col: token.col,
        })
    }

    fn parse_repeat(&mut self) -> LogoResult<AstNode> {
        let count = self.parse_expression()?;
        let body = self.parse_list_body()?;
        Ok(AstNode::Repeat {
            count: Box::new(count),
            body,
        })
    }

    fn parse_forever(&mut self) -> LogoResult<AstNode> {
        let body = self.parse_list_body()?;
        Ok(AstNode::Forever { body })
    }

    fn parse_launch(&mut self) -> LogoResult<AstNode> {
        let body = self.parse_list_body()?;
        Ok(AstNode::Launch { body })
    }

    fn parse_if(&mut self) -> LogoResult<AstNode> {
        let condition = self.parse_expression()?;
        let body = self.parse_list_body()?;
        Ok(AstNode::If {
            condition: Box::new(condition),
            body,
        })
    }

    fn parse_ifelse(&mut self) -> LogoResult<AstNode> {
        let condition = self.parse_expression()?;
        let then_body = self.parse_list_body()?;
        let else_body = self.parse_list_body()?;
        Ok(AstNode::IfElse {
            condition: Box::new(condition),
            then_body,
            else_body,
        })
    }

    fn parse_waituntil(&mut self) -> LogoResult<AstNode> {
        let condition = self.parse_list_body()?;
        Ok(AstNode::WaitUntil { condition })
    }

    fn parse_carefully(&mut self) -> LogoResult<AstNode> {
        let body = self.parse_list_body()?;
        let handler = self.parse_list_body()?;
        Ok(AstNode::Carefully { body, handler })
    }

    /// Parse [...] as a raw data list — bare words become word literals.
    fn parse_list(&mut self) -> LogoResult<AstNode> {
        self.expect(TokenType::OpenBracket)?;
        let mut elements = Vec::new();
        self.skip_newlines();
        while self.peek().token_type != TokenType::CloseBracket {
            if self.is_at_end() {
                return Err(LogoError::Syntax {
                    message: "Missing ']'".to_string(),
                    line: self.peek().line,
                    col: self.peek().col,
                });
            }
            elements.push(self.parse_raw_atom()?);
            self.skip_newlines();
        }
        self.expect(TokenType::CloseBracket)?;
        Ok(AstNode::List(elements))
    }

    /// Parse a single element inside a raw data list.
    fn parse_raw_atom(&mut self) -> LogoResult<AstNode> {
        let token = self.peek().clone();
        match token.token_type {
            TokenType::Number => {
                self.advance();
                Ok(AstNode::Number(token.value.parse::<f64>().unwrap()))
            }
            TokenType::QuotedWord => {
                self.advance();
                Ok(AstNode::Word(token.value))
            }
            TokenType::Variable => {
                self.advance();
                Ok(AstNode::Variable(token.value))
            }
            TokenType::Word => {
                self.advance();
                Ok(AstNode::Word(token.value))
            }
            TokenType::OpenBracket => self.parse_list(),
            _ => Err(LogoError::Syntax {
                message: format!("Unexpected token '{}' in list", token.value),
                line: token.line,
                col: token.col,
            }),
        }
    }

    /// Parse a [...] code block — returns inner AST nodes for execution.
    fn parse_list_body(&mut self) -> LogoResult<Vec<AstNode>> {
        self.expect(TokenType::OpenBracket)?;
        let mut body = Vec::new();
        self.skip_newlines();
        while self.peek().token_type != TokenType::CloseBracket {
            if self.is_at_end() {
                return Err(LogoError::Syntax {
                    message: "Missing ']'".to_string(),
                    line: self.peek().line,
                    col: self.peek().col,
                });
            }
            body.push(self.parse_expression()?);
            self.skip_newlines();
        }
        self.expect(TokenType::CloseBracket)?;
        Ok(body)
    }

    fn parse_paren_expr(&mut self) -> LogoResult<AstNode> {
        self.expect(TokenType::OpenParen)?;
        if self.peek().token_type == TokenType::Word {
            let name_token = self.advance();
            let name = name_token.value;
            let mut args = Vec::new();
            while self.peek().token_type != TokenType::CloseParen {
                if self.is_at_end() {
                    return Err(LogoError::Syntax {
                        message: "Missing ')'".to_string(),
                        line: name_token.line,
                        col: name_token.col,
                    });
                }
                args.push(self.parse_expression()?);
            }
            self.expect(TokenType::CloseParen)?;
            return Ok(AstNode::Paren { name, args });
        }
        let expr = self.parse_expression()?;
        self.expect(TokenType::CloseParen)?;
        Ok(expr)
    }

    // ── Helpers ─────────────────────────────

    fn peek(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn advance(&mut self) -> Token {
        let token = self.tokens[self.pos].clone();
        self.pos += 1;
        token
    }

    fn expect(&mut self, token_type: TokenType) -> LogoResult<Token> {
        let token = self.peek().clone();
        if token.token_type != token_type {
            return Err(LogoError::Syntax {
                message: format!("Expected {:?} but got '{}'", token_type, token.value),
                line: token.line,
                col: token.col,
            });
        }
        Ok(self.advance())
    }

    fn skip_newlines(&mut self) {
        while self.pos < self.tokens.len() && self.tokens[self.pos].token_type == TokenType::Newline
        {
            self.pos += 1;
        }
    }

    fn is_at_end(&self) -> bool {
        self.pos >= self.tokens.len() || self.tokens[self.pos].token_type == TokenType::Eof
    }
}

#[cfg(test)]
#[path = "tests/parser.rs"]
mod tests;
