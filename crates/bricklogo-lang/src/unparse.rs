use crate::ast::{AstNode, UserProcedure};

fn node_to_source(node: &AstNode) -> String {
    match node {
        AstNode::Number(n) => {
            if *n == n.floor() && n.is_finite() {
                format!("{}", *n as i64)
            } else {
                format!("{}", n)
            }
        }
        AstNode::Word(s) => format!("\"{}", s),
        AstNode::Variable(name) => format!(":{}", name),
        AstNode::List(elements) => {
            let inner = elements
                .iter()
                .map(node_to_source)
                .collect::<Vec<_>>()
                .join(" ");
            format!("[{}]", inner)
        }
        AstNode::Infix {
            operator,
            left,
            right,
        } => {
            format!(
                "{} {} {}",
                node_to_source(left),
                operator,
                node_to_source(right)
            )
        }
        AstNode::Paren { name, args } => {
            let arg_strs = args
                .iter()
                .map(node_to_source)
                .collect::<Vec<_>>()
                .join(" ");
            format!("({} {})", name, arg_strs)
        }
        AstNode::Call { name, args, .. } => {
            if args.is_empty() {
                name.clone()
            } else {
                let arg_strs = args
                    .iter()
                    .map(node_to_source)
                    .collect::<Vec<_>>()
                    .join(" ");
                format!("{} {}", name, arg_strs)
            }
        }
        AstNode::ProcDef { name, params, body } => procedure_to_source(&UserProcedure {
            name: name.clone(),
            params: params.clone(),
            body: body.clone(),
        }),
        AstNode::Repeat { count, body } => {
            format!(
                "repeat {} [{}]",
                node_to_source(count),
                body_to_source(body)
            )
        }
        AstNode::Forever { body } => {
            format!("forever [{}]", body_to_source(body))
        }
        AstNode::Launch { body } => {
            format!("launch [{}]", body_to_source(body))
        }
        AstNode::If { condition, body } => {
            format!(
                "if {} [{}]",
                node_to_source(condition),
                body_to_source(body)
            )
        }
        AstNode::IfElse {
            condition,
            then_body,
            else_body,
        } => {
            format!(
                "ifelse {} [{}] [{}]",
                node_to_source(condition),
                body_to_source(then_body),
                body_to_source(else_body)
            )
        }
        AstNode::WaitUntil { condition } => {
            format!("waituntil [{}]", body_to_source(condition))
        }
        AstNode::Carefully { body, handler } => {
            format!(
                "carefully [{}] [{}]",
                body_to_source(body),
                body_to_source(handler)
            )
        }
        AstNode::Output(value) => {
            format!("output {}", node_to_source(value))
        }
        AstNode::Stop => "stop".to_string(),
    }
}

fn body_to_source(body: &[AstNode]) -> String {
    body.iter()
        .map(node_to_source)
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn procedure_to_source(proc: &UserProcedure) -> String {
    let params = if proc.params.is_empty() {
        String::new()
    } else {
        format!(
            " {}",
            proc.params
                .iter()
                .map(|p| format!(":{}", p))
                .collect::<Vec<_>>()
                .join(" ")
        )
    };
    let header = format!("to {}{}", proc.name, params);
    let body = proc
        .body
        .iter()
        .map(|n| format!("  {}", node_to_source(n)))
        .collect::<Vec<_>>()
        .join("\n");
    format!("{}\n{}\nend", header, body)
}

pub fn procedures_to_source(procs: &[&UserProcedure]) -> String {
    let sources = procs
        .iter()
        .map(|p| procedure_to_source(p))
        .collect::<Vec<_>>();
    format!("{}\n", sources.join("\n\n"))
}

#[cfg(test)]
#[path = "tests/unparse.rs"]
mod tests;
