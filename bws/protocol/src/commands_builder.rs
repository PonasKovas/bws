use crate::datatypes::*;
use std::borrow::Cow;

pub struct CommandsBuilder<'a> {
    children: Vec<BuilderNode<'a>>,
}

impl<'a> CommandsBuilder<'a> {
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
        }
    }
    pub fn add(&mut self, node: BuilderNode<'a>) {
        self.children.push(node);
    }
    /// Returns a vector of command nodes ready to be used directly in the `PlayClientBound::DeclareCommands`
    /// with the root_node set to 0.
    pub fn build(self) -> Vec<CommandNode<'a>> {
        let mut res = Vec::new();

        // first add the root node
        // we will set it's children as we're adding them
        res.push(CommandNode::Root {
            children: Vec::new(),
        });

        add_children(&mut res, 0, self.children);

        res
    }
}

pub enum BuilderNode<'a> {
    Literal {
        name: Cow<'a, str>,
        executable: bool,
        children: Vec<BuilderNode<'a>>,
    },
    Argument {
        name: Cow<'a, str>,
        executable: bool,
        parser: Parser,
        suggestions: Option<SuggestionsType>,
        children: Vec<BuilderNode<'a>>,
    },
}

impl<'a> BuilderNode<'a> {
    pub fn add(&mut self, node: BuilderNode<'a>) {
        match self {
            Self::Literal { children, .. } => {
                children.push(node);
            }
            Self::Argument { children, .. } => {
                children.push(node);
            }
        }
    }
}

fn add_children<'a>(
    res: &mut Vec<CommandNode<'a>>,
    parent_id: usize,
    children: Vec<BuilderNode<'a>>,
) {
    for child in children {
        let my_index = res.len();

        // Add myself to the parent node
        match &mut res[parent_id] {
            CommandNode::Root { children } => {
                children.push(VarInt(my_index as i32));
            }
            CommandNode::Literal { children, .. } => {
                children.push(VarInt(my_index as i32));
            }
            CommandNode::Argument { children, .. } => {
                children.push(VarInt(my_index as i32));
            }
        }

        match child {
            BuilderNode::Literal {
                children,
                name,
                executable,
            } => {
                res.push(CommandNode::Literal {
                    executable,
                    children: Vec::new(),
                    redirect: None,
                    name,
                });
                add_children(res, my_index, children);
            }
            BuilderNode::Argument {
                children,
                name,
                executable,
                parser,
                suggestions,
            } => {
                res.push(CommandNode::Argument {
                    executable,
                    children: Vec::new(),
                    redirect: None,
                    name,
                    parser,
                    suggestions,
                });
                add_children(res, my_index, children);
            }
        }
    }
}
