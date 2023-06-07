use std::cell::RefCell;

use comrak::{
    nodes::{AstNode, NodeValue},
    Arena,
};
use indexmap::IndexMap;

#[derive(Debug)]
pub struct TocNode<'a> {
    level: u8,
    text: String,
    children: RefCell<Vec<&'a TocNode<'a>>>,
}

impl<'a> TocNode<'a> {
    pub fn build(
        arena: &'a Arena<TocNode<'a>>,
        chapters: &IndexMap<String, &'a AstNode<'a>>,
    ) -> &'a Self {
        fn build_toc_tree<'a>(
            arena: &'a Arena<TocNode<'a>>,
            stack: &mut Vec<&'a TocNode<'a>>,
            ast_node: &'a AstNode<'a>,
        ) {
            match &ast_node.data.borrow().value {
                NodeValue::Document => {
                    for child in ast_node.children() {
                        build_toc_tree(arena, stack, child);
                    }
                }
                NodeValue::Heading(heading) if heading.level < 3 => {
                    let mut text = None;
                    for child in ast_node.children() {
                        match &child.data.borrow().value {
                            NodeValue::Text(t) => {
                                if text.replace(t.to_owned()).is_some() {
                                    panic!("heading should have only a single text elements")
                                }
                            }
                            nv => panic!("unexpected node value {nv:?}"),
                        }
                    }

                    let toc = arena.alloc(TocNode {
                        level: heading.level,
                        text: text.expect("heading should have text"),
                        children: RefCell::new(vec![]),
                    });

                    stack.truncate(heading.level as usize);
                    stack.last_mut().unwrap().children.borrow_mut().push(toc);
                    stack.push(toc)
                }
                _ => {}
            };
        }

        let root = arena.alloc(TocNode {
            level: 0,
            text: String::new(),
            children: RefCell::new(vec![]),
        });

        let mut stack = vec![&*root];

        for (_, &ast_node) in chapters.iter() {
            let mut ast_node = ast_node;
            loop {
                build_toc_tree(arena, &mut stack, ast_node);
                let Some(n) = ast_node.next_sibling() else { break };
                ast_node = n;
            }
        }

        root
    }
}
